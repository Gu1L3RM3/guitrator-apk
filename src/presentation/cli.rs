use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::application::{ExtractApk, ListApps};
use crate::domain::AdbCommandExecutor;
use crate::infrastructure::download_apk;

#[derive(Parser)]
#[command(
    name = "guitrator-apk",
    about = "Extract APKs from Android devices via ADB"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all installed packages on the connected device.
    List,
    /// Pull an APK from the device to a local directory.
    Pull {
        /// Package name (e.g. com.example.app)
        #[arg(short, long)]
        package: String,
        /// Device APK path (from `list` output)
        #[arg(short, long)]
        apk_path: String,
        /// Destination directory
        #[arg(short, long, default_value = ".")]
        destination: PathBuf,
    },
    /// Download an APK from F-Droid by package name (no device needed).
    Download {
        /// Package name (e.g. org.fdroid.fdroid)
        #[arg(short, long)]
        package: String,
        /// Destination directory
        #[arg(short, long, default_value = ".")]
        destination: PathBuf,
        /// Also install it on the connected device via `adb install`
        #[arg(short, long)]
        install: bool,
    },
}

pub fn run(executor: &dyn AdbCommandExecutor) -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::List => {
            let apps = ListApps::new(executor).execute()?;
            if apps.is_empty() {
                println!("No packages found.");
            } else {
                println!("{:<60} APK PATH", "PACKAGE");
                println!("{}", "-".repeat(100));
                for app in &apps {
                    println!("{:<60} {}", app.package_name, app.apk_path);
                }
                println!("\n{} package(s) found.", apps.len());
            }
        }
        Commands::Pull {
            package,
            apk_path,
            destination,
        } => {
            let saved = ExtractApk::new(executor).execute(&apk_path, &destination, &package)?;
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
                executor.run(&["install", "-r", &path])?;
                println!("Installed {package}.");
            }
        }
    }
    Ok(())
}
