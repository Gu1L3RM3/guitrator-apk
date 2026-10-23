//! Parser for `dumpsys package <name>`: what Android knows about one installed app.
//!
//! Tolerant like the other parsers: fields a given Android version does not report stay
//! empty instead of failing.

/// Details of one installed app, shown when the user opens it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppDetails {
    pub version_name: Option<String>,
    pub version_code: Option<u64>,
    pub min_sdk: Option<u32>,
    pub target_sdk: Option<u32>,
    pub installer: Option<String>,
    pub first_install: Option<String>,
    pub last_update: Option<String>,
    pub data_dir: Option<String>,
    pub cpu_abi: Option<String>,
    pub system: bool,
    pub stopped: bool,
    pub disabled: bool,
    /// Every permission the app asks for.
    pub requested_permissions: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub denied_permissions: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Requested,
    Granting,
}

/// Reads the `Package [name]` block of `dumpsys package name`. Per-user values come from user 0.
pub fn parse_package_dump(output: &str, package: &str) -> AppDetails {
    let header = format!("Package [{package}]");
    let mut details = AppDetails::default();
    let mut lines = output
        .lines()
        .skip_while(|l| !l.trim_start().starts_with(&header))
        .skip(1);

    let mut user: Option<u32> = None;
    // The open permission list: its kind and the indent of its header.
    let mut section: Option<(Section, usize)> = None;

    for line in lines.by_ref() {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent < 4 {
            break; // next top-level block
        }
        if let Some((kind, header_indent)) = section {
            if indent > header_indent {
                add_permission(&mut details, kind, text);
                continue;
            }
            section = None;
        }

        match text {
            "requested permissions:" => section = Some((Section::Requested, indent)),
            "install permissions:" | "runtime permissions:" => {
                // Runtime grants are per user; only user 0 is read.
                if user.is_none() || user == Some(0) {
                    section = Some((Section::Granting, indent));
                }
            }
            _ if text.starts_with("User ") => {
                user = text[5..]
                    .split(|c: char| !c.is_ascii_digit())
                    .next()
                    .and_then(|n| n.parse().ok());
                if user == Some(0) {
                    details.stopped = field(text, "stopped=") == Some("true");
                    details.disabled = matches!(field(text, "enabled="), Some("2" | "3" | "4"));
                }
            }
            _ => read_field(&mut details, text, user),
        }
    }

    for list in [
        &mut details.requested_permissions,
        &mut details.granted_permissions,
        &mut details.denied_permissions,
    ] {
        list.sort();
        list.dedup();
    }
    details
}

fn read_field(details: &mut AppDetails, text: &str, user: Option<u32>) {
    let in_user_0 = user == Some(0);
    let in_header = user.is_none();
    if in_header {
        set_once(&mut details.version_name, text.strip_prefix("versionName="));
        set_once(
            &mut details.installer,
            text.strip_prefix("installerPackageName="),
        );
        set_once(
            &mut details.last_update,
            text.strip_prefix("lastUpdateTime="),
        );
        set_once(&mut details.cpu_abi, text.strip_prefix("primaryCpuAbi="));
        if text.starts_with("versionCode=") {
            details.version_code = field(text, "versionCode=").and_then(|v| v.parse().ok());
            details.min_sdk = field(text, "minSdk=").and_then(|v| v.parse().ok());
            details.target_sdk = field(text, "targetSdk=").and_then(|v| v.parse().ok());
        }
        if text.starts_with("flags=[") {
            details.system = text.split([' ', ']']).any(|flag| flag == "SYSTEM");
        }
    }
    if in_user_0 {
        set_once(&mut details.data_dir, text.strip_prefix("dataDir="));
        set_once(
            &mut details.first_install,
            text.strip_prefix("firstInstallTime="),
        );
    }
}

fn add_permission(details: &mut AppDetails, kind: Section, text: &str) {
    match kind {
        Section::Requested => details.requested_permissions.push(text.to_string()),
        Section::Granting => {
            let Some((name, rest)) = text.split_once(':') else {
                return;
            };
            if rest.contains("granted=true") {
                details.granted_permissions.push(name.to_string());
            } else if rest.contains("granted=false") {
                details.denied_permissions.push(name.to_string());
            }
        }
    }
}

/// The value after `key` in a line of space-separated `key=value` pairs.
fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let start = text.find(key)? + key.len();
    text[start..].split_whitespace().next()
}

/// Stores `value` unless one is already set, or it is the literal `null` Android prints.
fn set_once(slot: &mut Option<String>, value: Option<&str>) {
    if slot.is_none() {
        *slot = value
            .filter(|v| *v != "null" && !v.is_empty())
            .map(String::from);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str = "\
Shared users:
  something

Packages:
  Package [com.openai.chatgpt] (34100f1):
    appId=10270
    primaryCpuAbi=arm64-v8a
    versionCode=2628614 minSdk=32 targetSdk=37
    versionName=1.2026.286
    flags=[ HAS_CODE ALLOW_CLEAR_USER_DATA ]
    lastUpdateTime=2026-10-22 22:46:12
    installerPackageName=com.android.vending
    declared permissions:
      com.openai.own.PERMISSION: prot=signature
    requested permissions:
      android.permission.INTERNET
      android.permission.CAMERA
    install permissions:
      android.permission.INTERNET: granted=true
    User 0: ceDataInode=1 installed=true hidden=false suspended=false stopped=false notLaunched=false enabled=0 instant=false
      dataDir=/data/user/0/com.openai.chatgpt
      firstInstallTime=2026-02-05 19:14:16
      runtime permissions:
        android.permission.CAMERA: granted=false, flags=[ USER_SET]
        android.permission.POST_NOTIFICATIONS: granted=true, flags=[ USER_SET]
    User 10: ceDataInode=0 installed=false hidden=false stopped=true enabled=3
      dataDir=null
      firstInstallTime=1969-12-31 21:00:00
      runtime permissions:
        android.permission.SEND_SMS: granted=true
Queries:
  versionName=wrong
";

    #[test]
    fn reads_version_and_install_info() {
        let d = parse_package_dump(DUMP, "com.openai.chatgpt");
        assert_eq!(d.version_name.as_deref(), Some("1.2026.286"));
        assert_eq!(d.version_code, Some(2628614));
        assert_eq!((d.min_sdk, d.target_sdk), (Some(32), Some(37)));
        assert_eq!(d.installer.as_deref(), Some("com.android.vending"));
        assert_eq!(d.last_update.as_deref(), Some("2026-10-22 22:46:12"));
        assert_eq!(d.cpu_abi.as_deref(), Some("arm64-v8a"));
        assert!(!d.system);
    }

    #[test]
    fn reads_user_zero_data_and_ignores_other_users() {
        let d = parse_package_dump(DUMP, "com.openai.chatgpt");
        assert_eq!(
            d.data_dir.as_deref(),
            Some("/data/user/0/com.openai.chatgpt")
        );
        assert_eq!(d.first_install.as_deref(), Some("2026-02-05 19:14:16"));
        assert!(!d.stopped && !d.disabled);
        assert!(!d
            .granted_permissions
            .contains(&"android.permission.SEND_SMS".to_string()));
    }

    #[test]
    fn splits_permissions_into_requested_granted_and_denied() {
        let d = parse_package_dump(DUMP, "com.openai.chatgpt");
        assert_eq!(d.requested_permissions.len(), 2);
        assert_eq!(
            d.granted_permissions,
            [
                "android.permission.INTERNET",
                "android.permission.POST_NOTIFICATIONS"
            ]
        );
        assert_eq!(d.denied_permissions, ["android.permission.CAMERA"]);
    }

    #[test]
    fn stops_at_the_next_top_level_block() {
        let d = parse_package_dump(DUMP, "com.openai.chatgpt");
        assert_eq!(d.version_name.as_deref(), Some("1.2026.286"));
    }

    #[test]
    fn unknown_package_gives_empty_details() {
        assert_eq!(parse_package_dump(DUMP, "com.nope"), AppDetails::default());
        assert_eq!(parse_package_dump("", "x"), AppDetails::default());
    }

    #[test]
    fn flags_system_apps() {
        let dump = "  Package [a.b] (1):\n    flags=[ SYSTEM HAS_CODE ]\n";
        assert!(parse_package_dump(dump, "a.b").system);
    }
}
