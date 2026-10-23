use thiserror::Error;

/// One installed application on the Android device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInfo {
    pub package_name: String,
    /// Path to the APK on the device, as reported by `adb shell pm list packages -f`.
    pub apk_path: String,
    /// Size of the APK file in bytes. `None` when the device did not report it.
    pub apk_size: Option<u64>,
    /// Proportional set size of the app's running processes, in KiB. `None` when not running.
    pub memory_kb: Option<u64>,
}

impl AppInfo {
    pub fn new(package_name: impl Into<String>, apk_path: impl Into<String>) -> Self {
        Self {
            package_name: package_name.into(),
            apk_path: apk_path.into(),
            apk_size: None,
            memory_kb: None,
        }
    }

    /// Heuristic: apps preinstalled in read-only partitions.
    pub fn is_system(&self) -> bool {
        ["/system", "/product", "/vendor", "/apex", "/system_ext"]
            .iter()
            .any(|p| self.apk_path.starts_with(p))
    }
}

#[derive(Debug, Error)]
pub enum AdbError {
    #[error("ADB binary not found and auto-download failed: {0}")]
    AdbNotFound(String),
    #[error("ADB command failed (exit {code}): {stderr}")]
    CommandFailed { code: i32, stderr: String },
    #[error("Failed to parse ADB output: {0}")]
    ParseError(String),
    #[error(
        "No device connected. Connect the phone with a USB cable, enable USB debugging, \
         then run `adb devices` to confirm it is listed."
    )]
    NoDevice,
    #[error(
        "Device is unauthorized. Unlock the phone and accept the \"Allow USB debugging?\" \
         prompt (tick \"Always allow\"). If no prompt appears, open Developer options, tap \
         \"Revoke USB debugging authorizations\", then reconnect the cable."
    )]
    Unauthorized,
    #[error(
        "Device is offline. Unlock the phone, reconnect the USB cable, or restart the adb \
         server with `adb kill-server`."
    )]
    Offline,
    #[error(
        "More than one device is connected. Disconnect the extra devices so only one phone \
         remains, then try again."
    )]
    MultipleDevices,
    #[error("Package not installed: {0}")]
    PackageNotFound(String),
    #[error("Invalid package name: {0}")]
    InvalidPackageName(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Archive extraction error: {0}")]
    Archive(String),
}

/// Abstraction over running ADB commands. Real impl spawns a process; mock returns canned output.
pub trait AdbCommandExecutor: Send + Sync {
    fn run(&self, args: &[&str]) -> Result<String, AdbError>;

    /// Like `run`, but returns stdout untouched, for binary data (use with `adb exec-out`).
    /// The default goes through `run`, which is only safe for text.
    fn run_bytes(&self, args: &[&str]) -> Result<Vec<u8>, AdbError> {
        self.run(args).map(String::into_bytes)
    }
}

/// Parse `adb shell pm list packages -f` output into a list of AppInfo.
/// Each line: `package:/data/app/com.example-1/base.apk=com.example`
pub fn parse_package_list(output: &str) -> Result<Vec<AppInfo>, AdbError> {
    let mut apps = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let rest = line
            .strip_prefix("package:")
            .ok_or_else(|| AdbError::ParseError(format!("unexpected line: {line}")))?;
        let eq = rest
            .rfind('=')
            .ok_or_else(|| AdbError::ParseError(format!("missing '=' in: {line}")))?;
        apps.push(AppInfo::new(&rest[eq + 1..], &rest[..eq]));
    }
    Ok(apps)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "package:/data/app/com.example.app-1/base.apk=com.example.app\n\
                          package:/data/app/com.android.chrome-2/base.apk=com.android.chrome\n\
                          package:/system/app/Settings/Settings.apk=com.android.settings";

    #[test]
    fn parse_three_packages() {
        let apps = parse_package_list(SAMPLE).unwrap();
        assert_eq!(apps.len(), 3);
        assert_eq!(apps[0].package_name, "com.example.app");
        assert_eq!(apps[0].apk_path, "/data/app/com.example.app-1/base.apk");
        assert_eq!(apps[1].package_name, "com.android.chrome");
        assert_eq!(apps[2].package_name, "com.android.settings");
    }

    #[test]
    fn parse_empty_returns_empty_vec() {
        assert!(parse_package_list("").unwrap().is_empty());
    }

    #[test]
    fn parse_ignores_blank_lines() {
        let apps =
            parse_package_list("package:/data/app/com.foo-1/base.apk=com.foo\n\n   \n").unwrap();
        assert_eq!(apps.len(), 1);
    }

    #[test]
    fn parse_missing_prefix_returns_error() {
        assert!(matches!(
            parse_package_list("bad line"),
            Err(AdbError::ParseError(_))
        ));
    }

    #[test]
    fn parse_missing_equals_returns_error() {
        assert!(matches!(
            parse_package_list("package:/data/app/com.foo-1/base.apk"),
            Err(AdbError::ParseError(_))
        ));
    }
}
