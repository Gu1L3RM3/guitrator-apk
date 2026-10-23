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
        let output = Command::new(&self.adb_path)
            .args(args)
            .output()
            .map_err(AdbError::Io)?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            if stderr.contains("no devices") || stderr.contains("no emulators") {
                Err(AdbError::NoDevice)
            } else {
                Err(AdbError::CommandFailed { code, stderr })
            }
        }
    }
}
