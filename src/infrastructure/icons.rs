//! Extracts launcher icons from APK files.
//!
//! The icon is found by looking for launcher-named images in the APK's `res/` folder and
//! preferring the densest mipmap. This works for most apps. Apps whose icon is only an
//! adaptive XML drawable get no image and show a letter placeholder instead.

use std::fs;
use std::path::Path;

use crate::domain::{AdbCommandExecutor, AppInfo};

use super::apk_icon;

/// Icons are decoded and scaled once to this size, enough for the list rows.
const ICON_SIZE: u32 = 96;

/// Decoded icon, ready to upload as a texture.
pub struct IconPixels {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Returns the best icon image inside an APK, given `(entry name, uncompressed size)` pairs.
pub fn pick_icon_entry<'a>(entries: &[(&'a str, u64)]) -> Option<&'a str> {
    entries
        .iter()
        .filter(|(name, _)| {
            let lower = name.to_lowercase();
            lower.starts_with("res/")
                && (lower.ends_with(".png") || lower.ends_with(".webp"))
                && (lower.contains("launcher") || lower.contains("icon"))
        })
        .max_by_key(|(name, size)| {
            let lower = name.to_lowercase();
            (
                lower.contains("launcher"),
                lower.contains("mipmap"),
                density_rank(&lower),
                *size,
            )
        })
        .map(|(name, _)| *name)
}

fn density_rank(path: &str) -> u8 {
    // Check longer names first: "xhdpi" also contains "hdpi".
    [
        ("xxxhdpi", 5),
        ("xxhdpi", 4),
        ("xhdpi", 3),
        ("hdpi", 2),
        ("mdpi", 1),
    ]
    .iter()
    .find(|(tag, _)| path.contains(tag))
    .map_or(0, |(_, rank)| *rank)
}

/// Raw icon file (extension, bytes) found in an APK.
type IconFile = (&'static str, Vec<u8>);

/// Result of looking inside an APK: `Some(Some(_))` found, `Some(None)` it has no image icon,
/// `None` the APK could not be inspected (worth retrying later).
type Lookup = Option<Option<IconFile>>;

/// Returns the app's icon, using the cache in `cache_dir` when possible.
///
/// On a cache miss the icon is read straight out of the APK on the phone, which moves a few
/// KB. The APK itself is never copied to the computer. An APK without an image is remembered
/// as such; a failed lookup is not, so it is retried.
pub fn load_app_icon(
    executor: &dyn AdbCommandExecutor,
    app: &AppInfo,
    cache_dir: &Path,
) -> Option<IconPixels> {
    fs::create_dir_all(cache_dir).ok()?;
    let key = format!("{}-{}", app.package_name, app.apk_size.unwrap_or(0));

    if cache_dir.join(format!("{key}.none")).exists() {
        return None;
    }
    for ext in ["png", "webp"] {
        if let Ok(bytes) = fs::read(cache_dir.join(format!("{key}.{ext}"))) {
            return decode(&bytes).map(pixels);
        }
    }

    match read_icon_remotely(executor, &app.apk_path) {
        Some(Some((_, bytes))) => {
            let image = decode(&bytes)?;
            // Cache the already-scaled image: decoding it next launch is nearly free.
            let _ = image.save(cache_dir.join(format!("{key}.png")));
            Some(pixels(image))
        }
        Some(None) => {
            let _ = fs::write(cache_dir.join(format!("{key}.none")), b"");
            None
        }
        None => None,
    }
}

/// Reads the icon out of the APK on the phone with `unzip -p`, without copying the APK.
fn read_icon_remotely(executor: &dyn AdbCommandExecutor, apk_path: &str) -> Lookup {
    let read = |entry: &str| read_remote_entry(executor, apk_path, entry);
    // An unreadable manifest means `unzip -p` is unavailable or the APK is unreadable.
    read("AndroidManifest.xml")?;

    let entry = match apk_icon::find_icon_entry_with(read) {
        Some(entry) => entry,
        None => {
            let listing = list_remote_entries(executor, apk_path)?;
            let refs: Vec<(&str, u64)> = listing.iter().map(|(n, s)| (n.as_str(), *s)).collect();
            match pick_icon_entry(&refs) {
                Some(entry) => entry.to_string(),
                None => return Some(None),
            }
        }
    };
    let bytes = read(&entry)?;
    Some(Some((extension(&entry), bytes)))
}

/// Bytes of one entry of an APK on the phone. `None` when it is missing or unreadable.
fn read_remote_entry(
    executor: &dyn AdbCommandExecutor,
    apk_path: &str,
    entry: &str,
) -> Option<Vec<u8>> {
    // The command runs in the phone's shell, so refuse anything that could break the quoting.
    if apk_path.contains('\'') || entry.contains('\'') {
        return None;
    }
    let command = format!("unzip -p '{apk_path}' '{entry}'");
    let bytes = executor.run_bytes(&["exec-out", &command]).ok()?;
    (!bytes.is_empty()).then_some(bytes)
}

/// `(entry name, size)` of every file in an APK on the phone, from `unzip -l`.
fn list_remote_entries(
    executor: &dyn AdbCommandExecutor,
    apk_path: &str,
) -> Option<Vec<(String, u64)>> {
    if apk_path.contains('\'') {
        return None;
    }
    let command = format!("unzip -l '{apk_path}'");
    let output = executor.run(&["exec-out", &command]).ok()?;
    let entries = parse_unzip_listing(&output);
    (!entries.is_empty()).then_some(entries)
}

/// Parses `unzip -l` lines like `  1196  1981-01-01 01:01   res/drawable/a.xml`.
fn parse_unzip_listing(output: &str) -> Vec<(String, u64)> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let size = parts.next()?.parse().ok()?;
            let (_date, _time) = (parts.next()?, parts.next()?);
            let name = parts.collect::<Vec<_>>().join(" ");
            (!name.is_empty()).then_some((name, size))
        })
        .collect()
}

fn extension(entry: &str) -> &'static str {
    if entry.to_lowercase().ends_with(".webp") {
        "webp"
    } else {
        "png"
    }
}

fn decode(bytes: &[u8]) -> Option<image::RgbaImage> {
    let image = image::load_from_memory(bytes).ok()?;
    Some(
        image
            .resize(ICON_SIZE, ICON_SIZE, image::imageops::FilterType::Triangle)
            .to_rgba8(),
    )
}

fn pixels(image: image::RgbaImage) -> IconPixels {
    IconPixels {
        width: image.width() as usize,
        height: image.height() as usize,
        rgba: image.into_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_launcher_in_densest_mipmap() {
        let entries = [
            ("res/mipmap-hdpi/ic_launcher.png", 9000),
            ("res/mipmap-xxxhdpi/ic_launcher.png", 4000),
            ("res/mipmap-xhdpi/ic_launcher.png", 8000),
            ("res/drawable/logo_banner.png", 99999),
        ];
        assert_eq!(
            pick_icon_entry(&entries),
            Some("res/mipmap-xxxhdpi/ic_launcher.png")
        );
    }

    #[test]
    fn ignores_non_images_and_unrelated_names() {
        let entries = [
            ("res/layout/icon_row.xml", 10),
            ("assets/launcher.png", 10),
            ("res/drawable/background.png", 10),
        ];
        assert_eq!(pick_icon_entry(&entries), None);
    }

    #[test]
    fn falls_back_to_any_launcher_drawable() {
        let entries = [("res/drawable-xxhdpi/app_icon.webp", 500)];
        assert_eq!(
            pick_icon_entry(&entries),
            Some("res/drawable-xxhdpi/app_icon.webp")
        );
    }

    #[test]
    fn parses_unzip_listing() {
        let output = "Archive:  /x/base.apk\n  Length      Date    Time    Name\n\
                      ---------  ---------- -----   ----\n\
                         1196  1981-01-01 01:01   res/drawable/a.xml\n\
                       166719  1981-01-01 01:01   assets/my file.txt\n\
                      ---------                     -------\n";
        assert_eq!(
            parse_unzip_listing(output),
            vec![
                ("res/drawable/a.xml".to_string(), 1196),
                ("assets/my file.txt".to_string(), 166719),
            ]
        );
    }

    #[derive(Default)]
    struct RecordingFailure {
        calls: std::sync::Mutex<Vec<String>>,
    }
    impl AdbCommandExecutor for RecordingFailure {
        fn run(&self, args: &[&str]) -> Result<String, crate::domain::AdbError> {
            self.calls.lock().unwrap().push(args.join(" "));
            Err(crate::domain::AdbError::CommandFailed {
                code: 1,
                stderr: "unzip: not found".into(),
            })
        }
    }

    #[test]
    fn failed_remote_lookup_gives_up_without_pulling_the_apk() {
        let executor = RecordingFailure::default();
        assert!(read_icon_remotely(&executor, "/data/app/x/base.apk").is_none());

        let cache = std::env::temp_dir().join(format!("icons_nopull_{}", std::process::id()));
        let app = AppInfo::new("com.x", "/data/app/x/base.apk");
        assert!(load_app_icon(&executor, &app, &cache).is_none());
        assert!(!executor
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.contains("pull")));
        // A failure is not remembered, so the next launch retries.
        assert!(!cache.join("com.x-0.none").exists());
        let _ = fs::remove_dir_all(cache);
    }

    #[test]
    fn refuses_paths_that_could_break_shell_quoting() {
        assert!(read_remote_entry(
            &RecordingFailure::default(),
            "/data/app/it's/base.apk",
            "a.png"
        )
        .is_none());
        assert!(read_remote_entry(
            &RecordingFailure::default(),
            "/data/app/x/base.apk",
            "a'; rm -rf /"
        )
        .is_none());
    }
}
