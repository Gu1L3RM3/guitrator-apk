//! Downloads APKs from F-Droid (HTTPS, reproducibly built, signed repo) — no device needed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::domain::AdbError;

const FDROID: &str = "https://f-droid.org";

#[derive(Deserialize)]
struct PackageInfo {
    #[serde(rename = "suggestedVersionCode")]
    suggested_version_code: u64,
}

/// Downloads the F-Droid suggested version of `package` into `dest_dir`; returns the saved path.
pub fn download_apk(package: &str, dest_dir: &Path) -> Result<PathBuf, AdbError> {
    validate_package(package)?;
    let net = |e: reqwest::Error| AdbError::Network(e.to_string());

    let meta = reqwest::blocking::get(format!("{FDROID}/api/v1/packages/{package}"))
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.bytes())
        .map_err(net)?;
    let info: PackageInfo =
        serde_json::from_slice(&meta).map_err(|e| AdbError::ParseError(e.to_string()))?;

    let bytes = reqwest::blocking::get(format!(
        "{FDROID}/repo/{package}_{}.apk",
        info.suggested_version_code
    ))
    .and_then(|r| r.error_for_status())
    .and_then(|r| r.bytes())
    .map_err(net)?;

    verify_apk(&bytes)?;

    fs::create_dir_all(dest_dir).map_err(AdbError::Io)?;
    let local = dest_dir.join(format!("{package}.apk"));
    fs::write(&local, &bytes).map_err(AdbError::Io)?;
    Ok(local)
}

/// Package names go into URLs and file names, so only allow the Android charset.
fn validate_package(package: &str) -> Result<(), AdbError> {
    let ok = !package.is_empty()
        && package
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(AdbError::ParseError(format!(
            "invalid package name: {package}"
        )))
    }
}

// shortcut: only checks the APK is a zip with a manifest, not F-Droid's index sha256; upgrade to index-v2 hash check.
fn verify_apk(bytes: &[u8]) -> Result<(), AdbError> {
    zip::ZipArchive::new(io::Cursor::new(bytes))
        .and_then(|mut z| z.by_name("AndroidManifest.xml").map(|_| ()))
        .map_err(|e| AdbError::Archive(format!("downloaded file is not a valid APK: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_and_url_tricks() {
        for bad in ["", "../etc/passwd", "a/b", "a?b=c", "a b"] {
            assert!(validate_package(bad).is_err(), "{bad}");
        }
        assert!(validate_package("org.fdroid.fdroid").is_ok());
    }

    #[test]
    fn rejects_non_apk_bytes() {
        assert!(matches!(verify_apk(b"<html>"), Err(AdbError::Archive(_))));
    }
}
