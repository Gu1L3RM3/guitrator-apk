use thiserror::Error;

/// One installed application on the Android device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInfo {
    pub package_name: String,
    /// Path to the APK on the device, as reported by `adb shell pm list packages -f`.
    pub apk_path: String,
}

impl AppInfo {
    pub fn new(package_name: impl Into<String>, apk_path: impl Into<String>) -> Self {
        Self {
            package_name: package_name.into(),
            apk_path: apk_path.into(),
        }
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
    #[error("No device connected")]
    NoDevice,
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
