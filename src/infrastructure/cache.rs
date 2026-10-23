use std::path::PathBuf;

/// Per-user cache directory for downloaded platform-tools, icons and temporary APKs.
pub fn cache_dir() -> PathBuf {
    dirs_next::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("guitrator-apk")
}
