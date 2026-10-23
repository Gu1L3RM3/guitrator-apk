//! Plain-text reports about the phone and its apps, shared by the CLI and the MCP server so
//! both show the same thing.

use std::fmt::Write;

use crate::application::{ListApps, ReadAppDetails, ReadDeviceInfo};
use crate::domain::app_details::AppDetails;
use crate::domain::{AdbCommandExecutor, AdbError, AppInfo, DeviceInfo};
use crate::presentation::format;

/// Table of apps. `detailed` adds the APK size and RAM columns.
pub fn apps_table(apps: &[&AppInfo], detailed: bool) -> String {
    if apps.is_empty() {
        return "No packages found.\n".into();
    }
    let mut out = String::new();
    if detailed {
        let _ = writeln!(
            out,
            "{:<60} {:>9} {:>9}  APK PATH",
            "PACKAGE", "APK SIZE", "RAM"
        );
    } else {
        let _ = writeln!(out, "{:<60} APK PATH", "PACKAGE");
    }
    let _ = writeln!(out, "{}", "-".repeat(100));
    for app in apps {
        if detailed {
            let size = app.apk_size.map_or("-".into(), format::bytes);
            let ram = app.memory_kb.map_or("-".into(), format::kib);
            let _ = writeln!(
                out,
                "{:<60} {size:>9} {ram:>9}  {}",
                app.package_name, app.apk_path
            );
        } else {
            let _ = writeln!(out, "{:<60} {}", app.package_name, app.apk_path);
        }
    }
    let _ = writeln!(out, "\n{} package(s) found.", apps.len());
    out
}

/// Model, Android version, RAM and storage of the connected phone.
pub fn device_report(info: &DeviceInfo) -> String {
    let usage = |total: Option<u64>, free: Option<u64>| match (total, free) {
        (Some(total), Some(free)) => format!(
            "{} of {} used",
            format::kib(total - free.min(total)),
            format::kib(total)
        ),
        _ => "unknown".into(),
    };
    format!(
        "Model:    {}\nAndroid:  {}\nRAM:      {}\nStorage:  {}\n",
        info.model,
        info.android_version,
        usage(info.ram_total_kb, info.ram_available_kb),
        usage(info.storage_total_kb, info.storage_free_kb),
    )
}

/// Everything known about one app. `app` adds the size, memory and APK path from the app list.
pub fn app_report(package: &str, app: Option<&AppInfo>, d: &AppDetails) -> String {
    let mut out = String::new();
    let mut line = |label: &str, value: Option<String>| {
        if let Some(value) = value {
            let _ = writeln!(out, "{label:<18}{value}");
        }
    };
    line("Package:", Some(package.to_string()));
    line(
        "Version:",
        d.version_name.as_ref().map(|v| match d.version_code {
            Some(code) => format!("{v} (code {code})"),
            None => v.clone(),
        }),
    );
    line(
        "Android API:",
        match (d.min_sdk, d.target_sdk) {
            (Some(min), Some(target)) => Some(format!("min {min}, target {target}")),
            (None, Some(target)) => Some(format!("target {target}")),
            (Some(min), None) => Some(format!("min {min}")),
            (None, None) => None,
        },
    );
    line(
        "Type:",
        Some(if d.system { "system app" } else { "user app" }.into()),
    );
    line(
        "State:",
        app.map(|a| {
            if a.memory_kb.is_some() {
                "running"
            } else {
                "not running"
            }
            .into()
        }),
    );
    line("First installed:", d.first_install.clone());
    line("Last updated:", d.last_update.clone());
    line("Installed by:", d.installer.clone());
    line("APK size:", app.and_then(|a| a.apk_size).map(format::bytes));
    line(
        "Memory in use:",
        app.and_then(|a| a.memory_kb).map(format::kib),
    );
    line("CPU:", d.cpu_abi.clone());
    line("APK path:", app.map(|a| a.apk_path.clone()));
    line("Data folder:", d.data_dir.clone());

    for (title, list) in [
        ("Allowed", &d.granted_permissions),
        ("Denied", &d.denied_permissions),
        ("Requested", &d.requested_permissions),
    ] {
        if !list.is_empty() {
            let names: Vec<&str> = list
                .iter()
                .map(|p| p.rsplit('.').next().unwrap_or(p))
                .collect();
            let _ = writeln!(
                out,
                "\n{title} permissions ({}):\n  {}",
                list.len(),
                names.join(", ")
            );
        }
    }
    out
}

/// Reads the phone's summary and renders it.
pub fn read_device_report(executor: &dyn AdbCommandExecutor) -> Result<String, AdbError> {
    Ok(device_report(&ReadDeviceInfo::new(executor).execute()?))
}

/// Looks the app up on the phone, so a missing package is reported as such, and renders it.
pub fn read_app_report(
    executor: &dyn AdbCommandExecutor,
    package: &str,
) -> Result<String, AdbError> {
    let apps = ListApps::new(executor).execute_detailed()?;
    let app = apps
        .iter()
        .find(|a| a.package_name == package)
        .ok_or_else(|| AdbError::PackageNotFound(package.to_string()))?;
    let details = ReadAppDetails::new(executor).execute(package)?;
    Ok(app_report(package, Some(app), &details))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The value on the line that starts with `label`, ignoring column padding.
    pub(crate) fn value_of<'a>(report: &'a str, label: &str) -> &'a str {
        let line = report
            .lines()
            .find(|l| l.starts_with(label))
            .unwrap_or_else(|| panic!("no {label} line in:\n{report}"));
        line[label.len()..].trim()
    }

    fn app(name: &str, size: Option<u64>, ram: Option<u64>) -> AppInfo {
        let mut a = AppInfo::new(name, format!("/data/app/{name}/base.apk"));
        a.apk_size = size;
        a.memory_kb = ram;
        a
    }

    #[test]
    fn plain_table_keeps_the_original_layout() {
        let a = app("com.foo", Some(1), None);
        let table = apps_table(&[&a], false);
        assert!(table.starts_with(&format!("{:<60} APK PATH\n", "PACKAGE")));
        assert!(table.contains("/data/app/com.foo/base.apk"));
        assert!(!table.contains("APK SIZE"));
        assert!(table.ends_with("1 package(s) found.\n"));
    }

    #[test]
    fn detailed_table_shows_size_and_ram_or_a_dash() {
        let a = app("com.foo", Some(5 * 1024 * 1024), Some(2048));
        let b = app("com.bar", None, None);
        let table = apps_table(&[&a, &b], true);
        assert!(table.contains("APK SIZE"));
        assert!(table.contains("5 MB"));
        assert!(table.contains("2 MB"));
        let bar = table.lines().find(|l| l.starts_with("com.bar")).unwrap();
        assert!(bar.contains(" - "));
    }

    #[test]
    fn empty_table_says_so() {
        assert_eq!(apps_table(&[], true), "No packages found.\n");
    }

    #[test]
    fn device_report_shows_usage_or_unknown() {
        let info = DeviceInfo {
            model: "Pixel".into(),
            android_version: "14".into(),
            ram_total_kb: Some(4 * 1024 * 1024),
            ram_available_kb: Some(1024 * 1024),
            ..Default::default()
        };
        let report = device_report(&info);
        assert!(report.contains("Model:    Pixel"));
        assert!(report.contains("3.0 GB of 4.0 GB used"));
        assert!(report.contains("Storage:  unknown"));
    }

    #[test]
    fn app_report_lists_what_is_known_and_skips_the_rest() {
        let d = AppDetails {
            version_name: Some("1.2".into()),
            version_code: Some(12),
            granted_permissions: vec!["android.permission.CAMERA".into()],
            ..Default::default()
        };
        let report = app_report("com.foo", None, &d);
        assert_eq!(value_of(&report, "Version:"), "1.2 (code 12)");
        assert!(report.contains("Allowed permissions (1):\n  CAMERA"));
        assert!(!report.contains("Installed by"));
        assert!(!report.contains("Denied"));
    }
}
