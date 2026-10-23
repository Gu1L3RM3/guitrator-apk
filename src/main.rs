mod domain;
mod infrastructure;
mod application;
mod presentation;

use std::sync::Arc;

fn main() {
    // All logs go to stderr — keeps stdout clean for MCP mode.
    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Stderr)
        .init();

    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("mcp") => run_mcp(),
        Some("gui") | None => run_gui(),
        _ => run_cli(),
    }
}

fn build_executor() -> Arc<infrastructure::RealAdbRunner> {
    let cache_dir = dirs_next::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".cache"))
        .join("guitrator-apk");

    let adb_path = infrastructure::ensure_adb(&cache_dir).unwrap_or_else(|e| {
        eprintln!("Error locating adb: {e}");
        std::process::exit(1);
    });

    Arc::new(infrastructure::RealAdbRunner::new(adb_path))
}

fn run_gui() {
    let executor = build_executor();
    if let Err(e) = presentation::gui::launch(executor) {
        eprintln!("GUI error: {e}");
        std::process::exit(1);
    }
}

fn run_cli() {
    let executor = build_executor();
    if let Err(e) = presentation::cli::run(executor.as_ref()) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run_mcp() {
    let executor = build_executor();
    presentation::mcp::run(executor.as_ref());
}
