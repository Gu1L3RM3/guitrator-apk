use crate::domain::app_details::{parse_package_dump, AppDetails};
use crate::domain::{AdbCommandExecutor, AdbError};

/// Use case: read what Android reports about one installed app.
pub struct ReadAppDetails<'a> {
    executor: &'a dyn AdbCommandExecutor,
}

impl<'a> ReadAppDetails<'a> {
    pub fn new(executor: &'a dyn AdbCommandExecutor) -> Self {
        Self { executor }
    }

    pub fn execute(&self, package: &str) -> Result<AppDetails, AdbError> {
        // The name goes into the phone's shell; package names never need these characters.
        if !package
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
        {
            return Err(AdbError::InvalidPackageName(package.to_string()));
        }
        let output = self
            .executor
            .run(&["shell", &format!("dumpsys package {package}")])?;
        Ok(parse_package_dump(&output, package))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stub(&'static str);
    impl AdbCommandExecutor for Stub {
        fn run(&self, args: &[&str]) -> Result<String, AdbError> {
            assert_eq!(args, ["shell", "dumpsys package com.foo"]);
            Ok(self.0.to_string())
        }
    }

    #[test]
    fn queries_dumpsys_and_parses() {
        let dump = "  Package [com.foo] (1):\n    versionName=2.0\n";
        let details = ReadAppDetails::new(&Stub(dump)).execute("com.foo").unwrap();
        assert_eq!(details.version_name.as_deref(), Some("2.0"));
    }

    #[test]
    fn rejects_names_that_could_inject_shell_commands() {
        let result = ReadAppDetails::new(&Stub("")).execute("com.foo; reboot");
        assert!(matches!(result, Err(AdbError::InvalidPackageName(_))));
    }
}
