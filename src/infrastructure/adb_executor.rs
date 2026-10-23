use std::path::PathBuf;
use std::process::Command;

use crate::domain::{AdbCommandExecutor, AdbError};

/// Concrete runner that invokes the real `adb` binary.
pub struct RealAdbRunner {
    pub adb_path: PathBuf,
}

impl RealAdbRunner {
    pub fn new(adb_path: PathBuf) -> Self {
        Self { adb_path }
    }
}

impl AdbCommandExecutor for RealAdbRunner {
    fn run(&self, args: &[&str]) -> Result<String, AdbError> {
        self.run_bytes(args)
            .map(|stdout| String::from_utf8_lossy(&stdout).into_owned())
    }

    fn run_bytes(&self, args: &[&str]) -> Result<Vec<u8>, AdbError> {
        let output = Command::new(&self.adb_path)
            .args(args)
            .output()
            .map_err(AdbError::Io)?;

        if output.status.success() {
            Ok(output.stdout)
        } else {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            Err(classify_failure(code, stderr))
        }
    }
}

/// Maps adb's stderr to the most specific error so the user gets an actionable message.
fn classify_failure(code: i32, stderr: String) -> AdbError {
    let lower = stderr.to_lowercase();
    if lower.contains("no devices") || lower.contains("no emulators") {
        AdbError::NoDevice
    } else if lower.contains("unauthorized") {
        AdbError::Unauthorized
    } else if lower.contains("device offline") {
        AdbError::Offline
    } else if lower.contains("more than one device") {
        AdbError::MultipleDevices
    } else {
        AdbError::CommandFailed {
            code,
            stderr: stderr.trim().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unauthorized_device_is_classified_without_adb_noise() {
        let stderr = "adb: device unauthorized.\nThis adb server's $ADB_VENDOR_KEYS is not set\n";
        assert!(matches!(
            classify_failure(1, stderr.into()),
            AdbError::Unauthorized
        ));
    }

    #[test]
    fn no_devices_is_classified() {
        assert!(matches!(
            classify_failure(1, "error: no devices/emulators found\n".into()),
            AdbError::NoDevice
        ));
    }

    #[test]
    fn offline_device_is_classified() {
        assert!(matches!(
            classify_failure(1, "error: device offline\n".into()),
            AdbError::Offline
        ));
    }

    #[test]
    fn multiple_devices_are_classified() {
        assert!(matches!(
            classify_failure(1, "error: more than one device and emulator\n".into()),
            AdbError::MultipleDevices
        ));
    }

    #[test]
    fn other_failures_keep_trimmed_stderr() {
        match classify_failure(2, "  something broke \n".into()) {
            AdbError::CommandFailed { code, stderr } => {
                assert_eq!(code, 2);
                assert_eq!(stderr, "something broke");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
