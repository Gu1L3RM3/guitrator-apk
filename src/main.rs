mod application;
mod domain;
mod infrastructure;
mod presentation;

use std::sync::Arc;

use clap::Parser;

use domain::AdbCommandExecutor;
use presentation::cli::{Cli, Commands, McpTarget};

fn main() {
    // All logs go to stderr — keeps stdout clean for MCP mode.
    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Stderr)
        .init();

    // Parse before touching adb, so `--help` and usage errors work without a phone or adb.
    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Gui) {
        Commands::Gui => run_gui(),
        Commands::Mcp => run_mcp(),
        Commands::SetupMcp { target } => run_setup_mcp(target),
        command => run_cli(command),
    }
}

fn build_executor() -> Arc<dyn AdbCommandExecutor> {
    let adb_path = infrastructure::ensure_adb(&infrastructure::cache_dir()).unwrap_or_else(|e| {
        eprintln!("Could not find or download adb: {e}");
        eprintln!("Install Android platform-tools and make sure `adb` is on your PATH.");
        std::process::exit(1);
    });

    Arc::new(infrastructure::RealAdbRunner::new(adb_path))
}

fn run_gui() {
    if let Err(e) = presentation::gui::launch(build_executor()) {
        eprintln!("GUI error: {e}");
        std::process::exit(1);
    }
}

fn run_cli(command: Commands) {
    if let Err(e) = presentation::cli::run(command, build_executor) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run_mcp() {
    let executor = build_executor();
    presentation::mcp::run(executor.as_ref());
}

/// Registers this binary as an MCP server in the Claude Code or Codex CLI.
fn run_setup_mcp(target: McpTarget) {
    if let Some(app_id) = std::env::var_os("FLATPAK_ID") {
        eprintln!(
            "{}",
            presentation::flatpak_mcp_help(&app_id.to_string_lossy())
        );
        std::process::exit(1);
    }
    let exe = std::env::current_exe().expect("cannot locate own executable");
    let (cli, args) = presentation::setup_mcp_command(Some(target.name()), &exe.to_string_lossy())
        .expect("every McpTarget has a registration command");
    match std::process::Command::new(cli).args(&args).status() {
        Ok(s) if s.success() => println!("MCP server 'guitrator-apk' added to {cli}."),
        Ok(s) => std::process::exit(s.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("Could not run `{cli}` (is it installed and in PATH?): {e}");
            std::process::exit(1);
        }
    }
}
