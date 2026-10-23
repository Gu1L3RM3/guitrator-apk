use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};

use crate::application::{ExtractApk, ListApps};
use crate::domain::AdbCommandExecutor;
use crate::infrastructure::download_apk;
use crate::presentation::report;

const AFTER_HELP: &str = "\
EXAMPLES:
  List the apps installed on the connected phone:
    guitrator-apk list

  Same, with APK size and memory use, hiding system apps:
    guitrator-apk list --detailed --user-only

  Show the phone's model, Android version, RAM and storage:
    guitrator-apk device

  Show everything about one app (version, install dates, permissions...):
    guitrator-apk info -p com.example.app

  Pull an app's APK into ./apks (use the APK PATH column from `list`):
    guitrator-apk pull -p com.example.app -a /data/app/com.example.app-1/base.apk -d ./apks

  Download an app from F-Droid without a phone:
    guitrator-apk download -p org.fdroid.fdroid

  Download from F-Droid and install it on the connected phone:
    guitrator-apk download -p org.fdroid.fdroid --install

  Register the MCP server with Claude Code or Codex:
    guitrator-apk setup-mcp claude
    guitrator-apk setup-mcp codex

ENVIRONMENT:
  RUST_LOG    Diagnostic log level written to stderr, e.g. RUST_LOG=info or RUST_LOG=debug.

TROUBLESHOOTING:
  'Device is unauthorized'   Unlock the phone and accept the USB debugging prompt.
  'No device connected'      Use a data cable, enable USB debugging, then run `adb devices`.
  'Device is offline'        Reconnect the cable or run `adb kill-server`.

  adb is taken from PATH when available. Otherwise Android platform-tools are downloaded
  once into the local cache directory. `--help` never needs adb.

Project home: https://github.com/Gu1L3RM3/guitrator-apk";

#[derive(Parser)]
#[command(
    name = "guitrator-apk",
    version,
    about = "Extract APKs from Android phones over USB, or download them from F-Droid",
    long_about = "Guitrator APK gets Android app packages (APKs) in two ways:\n\n\
        \u{2022} From a phone over USB: list the installed apps and pull the APK of any of them.\n\
        \u{2022} From F-Droid: download the latest build of an open-source app, with no phone needed.\n\n\
        Running it with no command opens the graphical interface.",
    after_help = AFTER_HELP,
    propagate_version = true
)]
pub struct Cli {
    /// Command to run. Opens the graphical interface when omitted.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List installed apps on the connected phone
    #[command(
        long_about = "List every package installed on the connected phone, with the \
        path of its APK on the device.\n\nThe APK PATH column is the value to pass to \
        `pull --apk-path`. System apps are included unless --user-only is given."
    )]
    List {
        /// Also show each APK's size and the memory its processes use (a little slower)
        #[arg(long)]
        detailed: bool,

        /// Leave out preinstalled system apps
        #[arg(long)]
        user_only: bool,
    },

    /// Show the phone's model, Android version, RAM and storage
    Device,

    /// Show version, install dates, permissions and more about one app
    #[command(
        long_about = "Show what Android reports about one installed app: version, API \
        levels, install and update dates, installer, APK size, memory use, and the \
        permissions it requests, was granted and was denied.\n\nFails if the package is \
        not installed. Find package names with `list`."
    )]
    Info {
        /// Package name of the app, e.g. com.example.app
        #[arg(short, long, value_name = "PACKAGE")]
        package: String,
    },

    /// Pull an app's APK from the phone to this computer
    #[command(
        long_about = "Copy the APK of one installed app from the connected phone to a \
        local directory. The file is saved as <PACKAGE>.apk.\n\nGet the APK path with `list`."
    )]
    Pull {
        /// Package name of the app, e.g. com.example.app
        #[arg(short, long, value_name = "PACKAGE")]
        package: String,

        /// APK path on the device, as shown by `list`
        #[arg(short, long, value_name = "PATH")]
        apk_path: String,

        /// Directory to save the APK in; created if missing
        #[arg(short, long, default_value = ".", value_name = "DIR")]
        destination: PathBuf,
    },

    /// Download an app from F-Droid, optionally installing it on the phone
    #[command(
        long_about = "Download the latest suggested build of an open-source app from the \
        F-Droid repository over HTTPS. The file is checked to be a real APK before it is saved \
        as <PACKAGE>.apk.\n\nNo phone is needed unless --install is given."
    )]
    Download {
        /// Package name on F-Droid, e.g. org.fdroid.fdroid
        #[arg(short, long, value_name = "PACKAGE")]
        package: String,

        /// Directory to save the APK in; created if missing
        #[arg(short, long, default_value = ".", value_name = "DIR")]
        destination: PathBuf,

        /// Install the downloaded APK on the connected phone with `adb install -r`
        #[arg(short, long)]
        install: bool,
    },

    /// Open the graphical interface (the default when no command is given)
    Gui,

    /// Run as an MCP server over stdio, for AI coding assistants
    #[command(
        long_about = "Start the Model Context Protocol server on stdin/stdout. Assistants \
        use it to list installed apps, read phone and app details, pull APKs and download from F-Droid. Logs go to stderr so the protocol stream \
        stays clean.\n\nNormally started by the assistant, not by hand. Use `setup-mcp` to \
        register it."
    )]
    Mcp,

    /// Register this program as an MCP server in Claude Code or Codex
    #[command(
        long_about = "Add guitrator-apk as an MCP server by running the client's own \
        `mcp add` command, pointing it at this executable.\n\nThe client CLI must be installed \
        and on PATH."
    )]
    SetupMcp {
        /// Client to configure
        #[arg(value_enum, value_name = "CLIENT")]
        target: McpTarget,
    },
}

/// AI clients that `setup-mcp` can configure.
#[derive(Clone, Copy, ValueEnum)]
pub enum McpTarget {
    /// Claude Code, registered at user scope
    Claude,
    /// OpenAI Codex CLI
    Codex,
}

impl McpTarget {
    pub fn name(self) -> &'static str {
        match self {
            McpTarget::Claude => "claude",
            McpTarget::Codex => "codex",
        }
    }
}

/// Runs a command that talks to the phone. `connect` is only called when adb is needed,
/// so commands such as `download` without `--install` never touch adb.
///
/// `gui`, `mcp` and `setup-mcp` are dispatched by `main` before this is called.
pub fn run(
    command: Commands,
    connect: impl FnOnce() -> Arc<dyn AdbCommandExecutor>,
) -> anyhow::Result<()> {
    match command {
        Commands::List {
            detailed,
            user_only,
        } => {
            let executor = connect();
            let lister = ListApps::new(executor.as_ref());
            let apps = if detailed {
                lister.execute_detailed()?
            } else {
                lister.execute()?
            };
            let shown: Vec<_> = apps
                .iter()
                .filter(|a| !(user_only && a.is_system()))
                .collect();
            print!("{}", report::apps_table(&shown, detailed));
        }
        Commands::Device => {
            print!("{}", report::read_device_report(connect().as_ref())?);
        }
        Commands::Info { package } => {
            print!("{}", report::read_app_report(connect().as_ref(), &package)?);
        }
        Commands::Pull {
            package,
            apk_path,
            destination,
        } => {
            let executor = connect();
            let saved =
                ExtractApk::new(executor.as_ref()).execute(&apk_path, &destination, &package)?;
            println!("Saved: {}", saved.display());
        }
        Commands::Download {
            package,
            destination,
            install,
        } => {
            let saved = download_apk(&package, &destination)?;
            println!("Saved: {}", saved.display());
            if install {
                let path = saved.to_string_lossy();
                connect().run(&["install", "-r", &path])?;
                println!("Installed {package} on the connected phone.");
            }
        }
        Commands::Gui | Commands::Mcp | Commands::SetupMcp { .. } => {
            unreachable!("gui, mcp and setup-mcp are dispatched by main")
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn no_command_means_gui() {
        let cli = Cli::try_parse_from(["guitrator-apk"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn help_lists_every_command() {
        let help = Cli::command().render_long_help().to_string();
        for name in [
            "list",
            "device",
            "info",
            "pull",
            "download",
            "gui",
            "mcp",
            "setup-mcp",
            "EXAMPLES",
        ] {
            assert!(help.contains(name), "help is missing {name}");
        }
    }

    #[test]
    fn setup_mcp_rejects_unknown_client() {
        assert!(Cli::try_parse_from(["guitrator-apk", "setup-mcp", "vim"]).is_err());
    }

    #[test]
    fn list_flags_default_to_off_and_can_be_combined() {
        let plain = Cli::try_parse_from(["guitrator-apk", "list"]).unwrap();
        assert!(matches!(
            plain.command,
            Some(Commands::List {
                detailed: false,
                user_only: false
            })
        ));
        let both =
            Cli::try_parse_from(["guitrator-apk", "list", "--detailed", "--user-only"]).unwrap();
        assert!(matches!(
            both.command,
            Some(Commands::List {
                detailed: true,
                user_only: true
            })
        ));
    }

    #[test]
    fn info_requires_a_package() {
        assert!(Cli::try_parse_from(["guitrator-apk", "info"]).is_err());
        assert!(Cli::try_parse_from(["guitrator-apk", "info", "-p", "com.foo"]).is_ok());
    }

    #[test]
    fn pull_requires_package_and_apk_path() {
        assert!(Cli::try_parse_from(["guitrator-apk", "pull", "-p", "com.foo"]).is_err());
    }
}
