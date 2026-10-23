//! Integration tests — no real device needed (stub executor).

use crate::application::{ExtractApk, ListApps};
use crate::domain::{AdbCommandExecutor, AdbError};

struct StubExecutor {
    list_response: &'static str,
}

impl AdbCommandExecutor for StubExecutor {
    fn run(&self, args: &[&str]) -> Result<String, AdbError> {
        if args.contains(&"pm") {
            Ok(self.list_response.to_string())
        } else if args.contains(&"pull") {
            Ok(String::new())
        } else {
            Err(AdbError::CommandFailed {
                code: 1,
                stderr: "unknown command".into(),
            })
        }
    }
}

const SAMPLE: &str = "package:/data/app/com.example.one-1/base.apk=com.example.one\n\
                      package:/data/app/com.example.two-1/base.apk=com.example.two";

#[test]
fn end_to_end_list_and_extract() {
    let stub = StubExecutor {
        list_response: SAMPLE,
    };
    let apps = ListApps::new(&stub).execute().expect("list should succeed");
    assert_eq!(apps.len(), 2);
    assert_eq!(apps[0].package_name, "com.example.one");

    let tmp = std::env::temp_dir().join("guitrator_integration_test");
    let saved = ExtractApk::new(&stub)
        .execute(&apps[0].apk_path, &tmp, &apps[0].package_name)
        .expect("extract should succeed");

    assert!(saved.ends_with("com.example.one.apk"));
    let _ = std::fs::remove_dir_all(&tmp);
}
