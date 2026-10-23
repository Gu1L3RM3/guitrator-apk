# Guitrator APK

[![CI](https://github.com/Gu1L3RM3/guitrator-apk/actions/workflows/ci.yml/badge.svg)](https://github.com/Gu1L3RM3/guitrator-apk/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Guitrator APK gets Android app packages (APKs) for you in two ways:

- **From a phone over USB.** It lists the apps installed on the device and pulls the APK of any app you choose.
- **From F-Droid, with no phone.** It downloads the latest suggested build of an open-source app straight from the F-Droid repository, and can optionally install it on a connected phone.

It ships as a desktop **GUI**, a **CLI**, and an **MCP server** so AI coding assistants (Claude Code, Codex, and other MCP clients) can use it directly.

## The Phone tab

The Phone tab shows the connected phone's model, Android version, RAM and storage. Each app shows its icon, the size of its APK and, when it is running, the memory it uses.

## Features

- Browse installed apps on a connected phone, with each app's icon, APK size and memory use (RAM) when running.
- Phone summary: model, Android version, RAM and storage in use.
- Search, a "Hide system apps" filter (on by default), and sorting by name, APK size or memory.
- In-app **Help** tab with phone setup, a quick start and troubleshooting.
- App details screen: version, API levels, install and update dates, installer, size, memory and permissions.
- Refresh button (and **F5**) that reloads the phone info and app list without blanking the screen.
- Extract any app's APK to a folder you pick.
- Download open-source apps from F-Droid over HTTPS, with a basic check that the file is a real APK.
- Install a downloaded APK on the connected phone in one step (`--install`).
- Automatic setup of Android platform-tools (`adb`) when it is not already on your system.
- An MCP server that lets an AI assistant list apps, read phone and app details, extract APKs and download from F-Droid.
- The same information from the terminal: `list --detailed`, `device` and `info`.
- Prebuilt binaries for Linux and Windows, and a Flatpak bundle for Linux.

## Install

Download the file for your system from the [Releases](https://github.com/Gu1L3RM3/guitrator-apk/releases) page. No Rust or other toolchain is needed.

| File | How to run it |
|------|---------------|
| `guitrator-apk.flatpak` | Double-click it, or run `flatpak install guitrator-apk.flatpak` |
| `guitrator-apk-linux-x86_64` | `chmod +x guitrator-apk-linux-x86_64`, then double-click it or run it from a terminal |
| `guitrator-apk-windows-x86_64.exe` | Double-click it. Windows SmartScreen will warn because the binary is not code-signed; choose "Run anyway" if you trust the release. |

Every release also includes a `SHA256SUMS` file. To verify a download on Linux:

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

Only Linux (x86_64) and Windows (x86_64) builds are published. macOS is not currently supported as a prebuilt target; you can install it with Cargo (see below) if you have a Rust toolchain.

### Other ways to install

**Linux: prebuilt binary on your PATH**

Instead of double-clicking, copy the binary to a folder on your `PATH` and run it from any terminal:

```bash
install -Dm755 guitrator-apk-linux-x86_64 ~/.local/bin/guitrator-apk
guitrator-apk --help
```

`~/.local/bin` is on `PATH` on most distributions. If `guitrator-apk` is "command not found", add it to your shell profile: `export PATH="$HOME/.local/bin:$PATH"`.

**Cargo, straight from GitHub (Linux, Windows, macOS)**

If you have Rust 1.77+, Cargo can build and install the latest code in one command, without cloning the repo:

```bash
cargo install --git https://github.com/Gu1L3RM3/guitrator-apk --locked
```

The binary goes to Cargo's bin folder (`$CARGO_HOME/bin`, usually `~/.cargo/bin`). Make sure that folder is on your `PATH`. On Linux, install the GTK packages listed under [Build from source](#build-from-source) first.

**Cargo, from a local clone**

Use this if you want to hack on the code and install your local changes:

```bash
git clone https://github.com/Gu1L3RM3/guitrator-apk.git
cd guitrator-apk
cargo install --path . --locked
```

To update later, run `git pull` and then the same `cargo install --path . --locked` command again.

**Flatpak, built locally**

Build the bundle yourself instead of downloading the release file. The steps are in [Flatpak](#flatpak) under Build from source, and the result installs with `flatpak install guitrator-apk.flatpak`.

**Register it with your AI assistant after installing**

Any of the methods above puts the binary on your system. To use it from Claude Code or Codex, run `guitrator-apk setup-mcp claude` or `guitrator-apk setup-mcp codex` (see [MCP](#mcp-claude-code-codex-and-other-clients)).

### Flatpak notes

The Flatpak sandbox gives the app network access, USB access (`--device=all`, needed by `adb`), and read/write access to your **Downloads** folder only (`xdg-download`). When using the Flatpak, choose a destination inside `~/Downloads` for extracted or downloaded APKs. The host also needs Android udev rules so `adb` can see the phone. Install them on the host (on Debian/Ubuntu-based systems, the `android-sdk-platform-tools-common` package provides them), because the sandbox cannot add them.

## Phone setup

To extract apps from a phone:

1. On the phone, enable **Developer options** (Settings → About phone → tap *Build number* seven times).
2. In Developer options, enable **USB debugging**.
3. Connect the phone with a USB cable that carries data (not a charge-only cable).
4. Accept the **"Allow USB debugging?" prompt** on the phone. Tick "Always allow from this computer" to avoid repeating it.

The phone-related features need that prompt accepted. F-Droid downloads work without a phone.

## Use

### GUI

Run the app with no arguments, or with `gui`:

```bash
guitrator-apk
```

- **Phone tab:** click **Refresh** (top right, or press **F5**) to reload the phone info and the app list; the current list stays on screen while it reloads. Click an app's name or **Details** to see its version, install dates, installer, permissions and more, with **Extract APK** and **Copy package name** buttons. Type in the search box to filter by name or package. Untick **Hide system apps** to see preinstalled system packages. Use **Sort by** to order the list by name, APK size or memory. Click **Extract** next to an app and choose a folder.
- **Icons** load in the background after the list appears, and are cached in your user cache folder so later refreshes are instant. Apps whose icon is only a vector drawable show a coloured letter instead.
- **Help tab:** phone setup steps, a quick start, and a table of common errors with their fixes.
- **F-Droid tab:** type a package name (for example `org.fdroid.fdroid`), tick **Also install on the connected phone** if you want, then click **Download**. Pressing Enter also submits.
- The status bar shows what the app is doing and whether the phone is ready.

### CLI

```bash
guitrator-apk list
guitrator-apk list --detailed --user-only
guitrator-apk device
guitrator-apk info -p com.example.app
guitrator-apk pull -p com.example.app -a /data/app/~~abc==/com.example.app-xyz==/base.apk -d ~/Downloads
guitrator-apk download -p org.fdroid.fdroid -d ~/apks
guitrator-apk download -p org.fdroid.fdroid -d ~/apks --install
```

| Command | Options | What it does |
|---------|---------|--------------|
| `list` | `--detailed`, `--user-only` | Prints every installed package on the connected phone and its APK path on the device. `--detailed` adds each APK's size and the RAM it uses (a little slower); `--user-only` leaves out preinstalled system apps. |
| `device` | — | Prints the phone's model, Android version, and RAM and storage in use. |
| `info` | `-p, --package <name>` (required) | Prints everything Android reports about one app: version, API levels, install and update dates, installer, size, memory, and its allowed, denied and requested permissions. Fails if the package is not installed. |
| `pull` | `-p, --package <name>` (required), `-a, --apk-path <path>` (required), `-d, --destination <dir>` (default `.`) | Copies one APK from the phone into the destination folder. Take the `-a` value from the `list` output. |
| `download` | `-p, --package <name>` (required), `-d, --destination <dir>` (default `.`), `-i, --install` | Downloads the F-Droid build of the package as `<package>.apk`. With `--install`, runs `adb install -r` on the connected phone afterwards. |

Use `guitrator-apk --help` or `guitrator-apk <command> --help` for the full reference.

Notes on `download`:

- Only packages in the F-Droid repository can be downloaded. Package names are limited to letters, digits, `.` and `_`.
- The connection uses HTTPS. The downloaded file is checked to be a valid APK (a ZIP containing `AndroidManifest.xml`). It is **not** yet checked against F-Droid's signed index hash, so treat it with the same care as any file from the internet.
- The file is saved as `<destination>/<package>.apk`, overwriting an existing file with that name.

### MCP (Claude Code, Codex, and other clients)

The MCP server speaks JSON-RPC over stdio. Keep the binary in a stable location first (for example `~/.local/bin/`), because the registration stores its absolute path. Then register it:

```bash
guitrator-apk setup-mcp claude   # runs: claude mcp add --scope user guitrator-apk -- <binary> mcp
guitrator-apk setup-mcp codex    # runs: codex mcp add guitrator-apk -- <binary> mcp
```

Both need the respective CLI installed and on your `PATH`.

**Installed with Flatpak?** `setup-mcp` cannot run inside the sandbox, so register the app from a normal terminal instead:

```bash
claude mcp add --scope user guitrator-apk -- flatpak run io.github.Gu1L3RM3.GuitratorApk mcp
codex mcp add guitrator-apk -- flatpak run io.github.Gu1L3RM3.GuitratorApk mcp
```

The Flatpak can only write to your Downloads folder, so give `extract_apk` and `download_apk` a `destination` inside it.

For other clients (Cursor, Claude Desktop, and so on), add this entry to their MCP configuration:

```json
{
  "mcpServers": {
    "guitrator-apk": {
      "command": "/path/to/guitrator-apk",
      "args": ["mcp"]
    }
  }
}
```

What the server exposes:

| Kind | Name | Input | Description |
|------|------|-------|-------------|
| Resource | `adb://device/installed_apps` | — | Text list of installed packages and their device APK paths (`package → path`). |
| Tool | `list_apps` | `detailed` (default `true`), `include_system` (default `false`) | Installed apps as a table, with APK size and RAM unless `detailed` is `false`. |
| Tool | `device_info` | — | The phone's model, Android version, RAM and storage in use. |
| Tool | `app_details` | `package_name` (required) | Version, API levels, install dates, installer, size, memory and permissions of one app. |
| Tool | `extract_apk` | `package_name` (required), `apk_path` (required), `destination` (optional, default `.`) | Pulls an APK from the connected phone into `destination` and returns the saved path. |
| Tool | `download_apk` | `package_name` (required), `destination` (optional, default `.`) | Downloads an F-Droid build into `destination` and returns the saved path. No phone needed. Unlike the CLI, it never installs the app. |

Arguments the assistant gets wrong come back as a JSON-RPC `-32602` error, and phone or network failures as `-32000` with a readable message.

The server writes logs to stderr only, so stdout carries just the JSON-RPC messages.

## How `adb` is found

On startup the app looks for `adb` in this order:

1. An `adb` (or `adb.exe` on Windows) already on your `PATH`.
2. A copy previously downloaded to the cache folder, `~/.cache/guitrator-apk/platform-tools/` (on Linux; the platform's equivalent cache folder elsewhere).
3. If neither exists, it downloads Google's official platform-tools archive from `dl.google.com` and extracts it into the cache folder.

The automatic `adb` download needs network access, but only when `adb` is missing. F-Droid downloads always need network access.

## Troubleshooting

- **The phone does not appear / "no devices":** check the cable, that USB debugging is on, and that you accepted the prompt on the phone. Run `adb devices` to see the state.
- **The phone shows as `unauthorized`:** unlock the phone and accept the "Allow USB debugging?" prompt, or revoke the USB debugging authorizations in Developer options and reconnect.
- **Extracted an app but it is missing from the list:** system apps are hidden by default in the GUI. Untick **Hide system apps**.
- **`Error locating adb`:** the automatic download failed (usually no network). Install `adb` yourself (`android-sdk-platform-tools` on most Linux distributions, or the platform-tools zip from Google) and make sure it is on your `PATH`.
- **Flatpak cannot write the APK:** pick a destination inside `~/Downloads`.
- **An app shows a letter instead of its icon:** the icon is a vector drawable, which is not read yet. Apps whose icon is inside the APK's resources are read automatically.
- **`setup-mcp` fails with "Could not run ...":** install the Claude Code or Codex CLI and make sure it is on your `PATH`, or register the server manually with the JSON above.
- **Windows SmartScreen warning:** expected for the unsigned binary; see [Install](#install).

## Build from source

You need Rust 1.77 or newer. On Linux also install the GTK and related development packages:

```bash
sudo apt-get install libgtk-3-dev libxkbcommon-dev libssl-dev pkg-config   # Debian / Ubuntu / Pop!_OS
```

```bash
git clone https://github.com/Gu1L3RM3/guitrator-apk.git
cd guitrator-apk
cargo build --release     # binary at target/release/guitrator-apk
cargo run                 # GUI, from source
cargo test
```

Before opening a pull request, the same checks CI runs:

```bash
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

### Flatpak

Tagged releases build the `.flatpak` automatically. To build it locally you need `flatpak-builder` and the committed `Cargo.lock`:

```bash
flatpak install flathub org.gnome.Platform//51 org.gnome.Sdk//51 org.freedesktop.Sdk.Extension.rust-stable//26.08
pip install aiohttp toml tomlkit
curl -O https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
python3 flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
flatpak-builder --user --force-clean --repo=repo build-dir packaging/flatpak/io.github.Gu1L3RM3.GuitratorApk.yml
flatpak build-bundle repo guitrator-apk.flatpak io.github.Gu1L3RM3.GuitratorApk
```

## Project layout

```
src/
├── domain/          types (AppInfo, DeviceInfo, AppDetails, AdbError), parsers and the AdbCommandExecutor trait
├── application/     use cases: ListApps, ReadDeviceInfo, ReadAppDetails, ExtractApk
├── infrastructure/  real adb runner, adb downloader, F-Droid download, APK icon reader and cache
├── presentation/    GUI (egui) with details screen and help tab, CLI (clap), MCP server, shared text reports
└── tests/           integration tests
```

Dependencies point inward: `presentation` and `infrastructure` depend on `application` and `domain`, never the reverse. The real `adb` runner sits behind the `AdbCommandExecutor` trait, so tests use a stub and need no phone.

## Contributing

Bug reports, ideas and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and PR guidelines, and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). To report a security issue, follow [SECURITY.md](SECURITY.md) instead of opening a public issue.

Changes are listed in [CHANGELOG.md](CHANGELOG.md).

## License

Released under the [MIT License](LICENSE).
