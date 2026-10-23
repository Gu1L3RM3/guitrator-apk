use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::{AdbCommandExecutor, AdbError};

/// Use case: pull the APK of `package_name` to `destination_dir`.
/// Returns the local path of the saved APK file.
pub struct ExtractApk<'a> {
    executor: &'a dyn AdbCommandExecutor,
}

impl<'a> ExtractApk<'a> {
    pub fn new(executor: &'a dyn AdbCommandExecutor) -> Self {
        Self { executor }
    }

    pub fn execute(
        &self,
        device_apk_path: &str,
        destination_dir: &Path,
        package_name: &str,
    ) -> Result<PathBuf, AdbError> {
        fs::create_dir_all(destination_dir).map_err(AdbError::Io)?;

        let local_file = destination_dir.join(format!("{package_name}.apk"));
        let local_str = local_file.to_str().ok_or_else(|| {
            AdbError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "non-UTF-8 destination path",
            ))
        })?;

        self.executor.run(&["pull", device_apk_path, local_str])?;
        Ok(local_file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct SpyExecutor {
        calls: Mutex<Vec<Vec<String>>>,
        fail: bool,
    }

    impl SpyExecutor {
        fn ok() -> Self {
            Self {
                calls: Mutex::new(vec![]),
                fail: false,
            }
        }
        fn fail() -> Self {
            Self {
                calls: Mutex::new(vec![]),
                fail: true,
            }
        }
    }

    impl AdbCommandExecutor for SpyExecutor {
        fn run(&self, args: &[&str]) -> Result<String, AdbError> {
            self.calls
                .lock()
                .unwrap()
                .push(args.iter().map(|s| s.to_string()).collect());
            if self.fail {
                Err(AdbError::CommandFailed {
                    code: 1,
                    stderr: "fail".into(),
                })
            } else {
                Ok(String::new())
            }
        }
    }

    #[test]
    fn calls_adb_pull_with_correct_args() {
        let spy = SpyExecutor::ok();
        let tmp = std::env::temp_dir().join("guitrator_test_extract");
        let result = ExtractApk::new(&spy).execute("/data/app/com.foo-1/base.apk", &tmp, "com.foo");
        assert!(result.is_ok());
        let calls = spy.calls.lock().unwrap();
        assert_eq!(calls[0][0], "pull");
        assert_eq!(calls[0][1], "/data/app/com.foo-1/base.apk");
        assert!(calls[0][2].ends_with("com.foo.apk"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn returns_error_on_adb_failure() {
        let spy = SpyExecutor::fail();
        let tmp = std::env::temp_dir().join("guitrator_test_fail");
        let result = ExtractApk::new(&spy).execute("/data/app/com.foo-1/base.apk", &tmp, "com.foo");
        assert!(matches!(result, Err(AdbError::CommandFailed { .. })));
        let _ = fs::remove_dir_all(&tmp);
    }
}
