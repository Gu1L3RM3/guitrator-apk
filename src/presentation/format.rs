//! Human-readable sizes for the UI.

/// Formats a byte count as B, KB, MB or GB, with one decimal only for GB.
pub fn bytes(value: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = value as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{value} B")
    }
}

/// Formats a size given in KiB, as reported by `/proc/meminfo` and `dumpsys`.
pub fn kib(value: u64) -> String {
    bytes(value * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_a_readable_unit() {
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(2048), "2 KB");
        assert_eq!(bytes(5 * 1024 * 1024), "5 MB");
        assert_eq!(bytes(3 * 1024 * 1024 * 1024 + 512 * 1024 * 1024), "3.5 GB");
        assert_eq!(kib(1024), "1 MB");
    }
}
