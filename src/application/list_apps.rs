use crate::domain::device_stats::{parse_apk_sizes, parse_pss_by_package};
use crate::domain::{parse_package_list, AdbCommandExecutor, AdbError, AppInfo};

/// Use case: list all installed packages on the connected device.
pub struct ListApps<'a> {
    executor: &'a dyn AdbCommandExecutor,
}

impl<'a> ListApps<'a> {
    pub fn new(executor: &'a dyn AdbCommandExecutor) -> Self {
        Self { executor }
    }

    pub fn execute(&self) -> Result<Vec<AppInfo>, AdbError> {
        let output = self
            .executor
            .run(&["shell", "pm", "list", "packages", "-f"])?;
        parse_package_list(&output)
    }

    /// Like `execute`, plus APK sizes and memory use. Those extra queries are best effort:
    /// if the device refuses one, the affected fields stay `None` and the list still loads.
    pub fn execute_detailed(&self) -> Result<Vec<AppInfo>, AdbError> {
        let mut apps = self.execute()?;
        if apps.is_empty() {
            return Ok(apps);
        }

        let quoted: Vec<String> = apps.iter().map(|a| format!("'{}'", a.apk_path)).collect();
        let stat = format!("stat -c '%s %n' {} 2>/dev/null; true", quoted.join(" "));
        match self.executor.run(&["shell", &stat]) {
            Ok(output) => {
                let sizes = parse_apk_sizes(&output);
                for app in &mut apps {
                    app.apk_size = sizes.get(&app.apk_path).copied();
                }
            }
            Err(e) => log::warn!("could not read APK sizes: {e}"),
        }

        match self.executor.run(&["shell", "dumpsys meminfo"]) {
            Ok(output) => {
                let memory = parse_pss_by_package(&output);
                for app in &mut apps {
                    app.memory_kb = memory.get(&app.package_name).copied();
                }
            }
            Err(e) => log::warn!("could not read memory use: {e}"),
        }

        Ok(apps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubExecutor(String);
    impl AdbCommandExecutor for StubExecutor {
        fn run(&self, _args: &[&str]) -> Result<String, AdbError> {
            Ok(self.0.clone())
        }
    }

    struct FailExecutor;
    impl AdbCommandExecutor for FailExecutor {
        fn run(&self, _args: &[&str]) -> Result<String, AdbError> {
            Err(AdbError::NoDevice)
        }
    }

    #[test]
    fn parses_executor_output() {
        let stub = StubExecutor(
            "package:/data/app/com.foo-1/base.apk=com.foo\n\
             package:/data/app/com.bar-1/base.apk=com.bar"
                .into(),
        );
        let apps = ListApps::new(&stub).execute().unwrap();
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].package_name, "com.foo");
        assert_eq!(apps[1].package_name, "com.bar");
    }

    #[test]
    fn propagates_no_device_error() {
        let result = ListApps::new(&FailExecutor).execute();
        assert!(matches!(result, Err(AdbError::NoDevice)));
    }
}
