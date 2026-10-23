//! Auto-downloads Android Platform Tools (adb) from Google if not in PATH.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::domain::AdbError;

#[cfg(target_os = "windows")]
const PLATFORM_TOOLS_URL: &str =
    "https://dl.google.com/android/repository/platform-tools-latest-windows.zip";
#[cfg(not(target_os = "windows"))]
const PLATFORM_TOOLS_URL: &str =
    "https://dl.google.com/android/repository/platform-tools-latest-linux.zip";

/// Returns the `adb` binary path, downloading platform-tools if needed.
///
/// Search order:
/// 1. `adb` already in PATH.
/// 2. Previously-downloaded copy in `<cache_dir>/platform-tools/`.
/// 3. Fresh download from Google into `<cache_dir>`.
pub fn ensure_adb(cache_dir: &Path) -> Result<PathBuf, AdbError> {
    if let Ok(path) = which_adb() {
        log::info!("adb found in PATH: {}", path.display());
        return Ok(path);
    }

    let local = local_adb_path(cache_dir);
    if local.exists() {
        log::info!("adb found in cache: {}", local.display());
        return Ok(local);
    }

    log::info!("adb not found; downloading from {PLATFORM_TOOLS_URL}");
    download_and_extract(cache_dir)?;

    if local.exists() {
        Ok(local)
    } else {
        Err(AdbError::AdbNotFound("adb missing after extraction".into()))
    }
}

fn which_adb() -> Result<PathBuf, ()> {
    let name = if cfg!(target_os = "windows") { "adb.exe" } else { "adb" };
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|p| p.join(name))
                .find(|p| p.exists())
        })
        .ok_or(())
}

fn local_adb_path(cache_dir: &Path) -> PathBuf {
    let bin = if cfg!(target_os = "windows") { "adb.exe" } else { "adb" };
    cache_dir.join("platform-tools").join(bin)
}

fn download_and_extract(cache_dir: &Path) -> Result<(), AdbError> {
    fs::create_dir_all(cache_dir)
        .map_err(|e| AdbError::Archive(format!("cannot create cache dir: {e}")))?;

    let bytes = reqwest::blocking::get(PLATFORM_TOOLS_URL)
        .and_then(|r| r.bytes())
        .map_err(|e| AdbError::Network(e.to_string()))?;

    let cursor = io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| AdbError::Archive(format!("zip open failed: {e}")))?;

    archive
        .extract(cache_dir)
        .map_err(|e| AdbError::Archive(format!("zip extract failed: {e}")))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let adb = local_adb_path(cache_dir);
        if adb.exists() {
            fs::set_permissions(&adb, fs::Permissions::from_mode(0o755))
                .map_err(AdbError::Io)?;
        }
    }

    Ok(())
}
