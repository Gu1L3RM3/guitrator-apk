//! Parsers for the stats that adb reports about the phone and its apps.
//!
//! Every parser is tolerant: lines it does not understand are skipped, so a different
//! Android version or vendor build degrades to missing values instead of an error.

use std::collections::HashMap;

/// Hardware and software summary of the connected phone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    pub model: String,
    pub android_version: String,
    pub ram_total_kb: Option<u64>,
    pub ram_available_kb: Option<u64>,
    /// Total and free space of the user data partition, in KiB.
    pub storage_total_kb: Option<u64>,
    pub storage_free_kb: Option<u64>,
}

/// Parses `stat -c '%s %n'` output into a map from file path to size in bytes.
pub fn parse_apk_sizes(output: &str) -> HashMap<String, u64> {
    output
        .lines()
        .filter_map(|line| {
            let (size, path) = line.trim().split_once(' ')?;
            Some((path.to_string(), size.parse().ok()?))
        })
        .collect()
}

/// Sums the PSS (KiB) of every process per package, from `dumpsys meminfo` output.
///
/// Only the "Total PSS by process" section is read, so the same memory is not counted twice.
/// Process names such as `com.example:remote` are attributed to `com.example`.
pub fn parse_pss_by_package(output: &str) -> HashMap<String, u64> {
    let mut totals = HashMap::new();
    let mut in_process_section = false;
    for line in output.lines() {
        // Section headers are not indented; process entries are.
        if !line.starts_with(char::is_whitespace) && !line.trim().is_empty() {
            in_process_section = line.starts_with("Total PSS by process");
            continue;
        }
        if !in_process_section {
            continue;
        }
        let Some((kb, rest)) = line.trim().split_once("K: ") else {
            continue;
        };
        let Some((name, _)) = rest.split_once(" (pid") else {
            continue;
        };
        let Ok(kb) = kb.replace(',', "").parse::<u64>() else {
            continue;
        };
        let package = name.split(':').next().unwrap_or(name).trim();
        *totals.entry(package.to_string()).or_insert(0) += kb;
    }
    totals
}

/// Reads `MemTotal` and `MemAvailable` (KiB) from `/proc/meminfo`.
pub fn parse_meminfo(output: &str) -> (Option<u64>, Option<u64>) {
    let field = |key: &str| {
        output.lines().find_map(|line| {
            line.strip_prefix(key)?
                .trim()
                .strip_suffix("kB")?
                .trim()
                .parse()
                .ok()
        })
    };
    (field("MemTotal:"), field("MemAvailable:"))
}

/// Reads total and free KiB from `df -k <path>` output. Uses the last data line.
pub fn parse_df(output: &str) -> (Option<u64>, Option<u64>) {
    let Some(columns) = output
        .lines()
        .rev()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .find(|cols| cols.len() >= 4)
    else {
        return (None, None);
    };
    (columns[1].parse().ok(), columns[3].parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apk_sizes_map_path_to_bytes() {
        let sizes = parse_apk_sizes("12345 /data/app/a/base.apk\n678 /system/app/b.apk\nbad\n");
        assert_eq!(sizes.get("/data/app/a/base.apk"), Some(&12345));
        assert_eq!(sizes.get("/system/app/b.apk"), Some(&678));
        assert_eq!(sizes.len(), 2);
    }

    #[test]
    fn pss_sums_processes_of_the_same_package() {
        let output = "\
Applications Memory Usage (in Kilobytes):
Uptime: 1000 Realtime: 1000

Total PSS by process:
    204,512K: com.android.systemui (pid 1234 / activities)
     10,000K: com.example.app (pid 2000)
      5,000K: com.example.app:remote (pid 2001)

Total PSS by OOM adjustment:
     99,999K: Native (pid 1)
";
        let totals = parse_pss_by_package(output);
        assert_eq!(totals.get("com.android.systemui"), Some(&204512));
        assert_eq!(totals.get("com.example.app"), Some(&15000));
        assert_eq!(totals.get("Native"), None);
    }

    #[test]
    fn pss_is_empty_when_no_process_section() {
        assert!(parse_pss_by_package("nothing useful here").is_empty());
    }

    #[test]
    fn meminfo_reads_total_and_available() {
        let output = "MemTotal:        7812345 kB\nMemFree:  100 kB\nMemAvailable:    3456789 kB\n";
        assert_eq!(parse_meminfo(output), (Some(7812345), Some(3456789)));
        assert_eq!(parse_meminfo(""), (None, None));
    }

    #[test]
    fn df_reads_total_and_free() {
        let output = "Filesystem     1K-blocks      Used Available Use% Mounted on\n\
                      /dev/block/dm-0 113000000 80000000  33000000  71% /data\n";
        assert_eq!(parse_df(output), (Some(113000000), Some(33000000)));
        assert_eq!(parse_df("not df output"), (None, None));
    }
}
