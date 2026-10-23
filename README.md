# Guitrator APK

[![CI](https://github.com/Gu1L3RM3/guitrator-apk/actions/workflows/ci.yml/badge.svg)](https://github.com/Gu1L3RM3/guitrator-apk/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Extract APKs from an Android phone over USB, or download them from F-Droid with no phone at all. Comes as a GUI, a CLI and an MCP server.

If `adb` is not in your `PATH`, the app downloads Android platform-tools on first run.

## Install

Download from the [Releases](https://github.com/Gu1L3RM3/guitrator-apk/releases) page. No Rust needed.

| File | How |
|------|-----|
| `guitrator-apk.flatpak` | Double-click, or `flatpak install guitrator-apk.flatpak` |
| `guitrator-apk-linux-x86_64` | Make it executable (`chmod +x`), then double-click |
| `guitrator-apk-windows-x86_64.exe` | Double-click (SmartScreen will warn: the binary is unsigned) |

Double-clicking opens the GUI. For phone features, enable **USB debugging** and accept the prompt on the phone. With Flatpak, your host also needs the Android udev rules so `adb` can see the phone.

## Use

### GUI

Connect the phone, click **Refresh**, search, then click **Extract** next to an app and pick a folder.

### CLI

```bash
guitrator-apk list                                           # installed packages on the phone
guitrator-apk pull -p com.example.app -a /data/app/.../base.apk -d ~/Downloads
guitrator-apk download -p org.fdroid.fdroid -d ~/apks        # from F-Droid, no phone needed
guitrator-apk download -p org.fdroid.fdroid --install        # also install on the connected phone
```

`download` only accepts F-Droid packages, over HTTPS. It checks the file is a valid APK but does not verify F-Droid's index hash.

### MCP (Claude Code, Codex)

Keep the binary where it will stay, then register it:

```bash
guitrator-apk setup-mcp claude   # claude mcp add --scope user guitrator-apk -- <binary> mcp
guitrator-apk setup-mcp codex    # codex mcp add guitrator-apk -- <binary> mcp
```

For other clients (Cursor, Claude Desktop), add this to their MCP config:

```json
{ "mcpServers": { "guitrator-apk": { "command": "/path/to/guitrator-apk", "args": ["mcp"] } } }
```

| Kind | Name | Description |
|------|------|-------------|
| Resource | `adb://device/installed_apps` | Installed packages and their device APK paths |
| Tool | `extract_apk` | `package_name`, `apk_path`, optional `destination` |

## Build from source

Needs Rust 1.77+. On Linux also `libgtk-3-dev libxkbcommon-dev libssl-dev pkg-config`.

```bash
cargo build --release     # binary at target/release/guitrator-apk
cargo test
```

### Flatpak

Tagged releases build the `.flatpak` automatically. To build it locally you need `flatpak-builder` and a committed `Cargo.lock`:

```bash
flatpak install flathub org.gnome.Platform//47 org.gnome.Sdk//47 org.freedesktop.Sdk.Extension.rust-stable//24.08
pip install aiohttp toml tomlkit
curl -O https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
python3 flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
flatpak-builder --user --force-clean --repo=repo build-dir packaging/flatpak/io.github.Gu1L3RM3.GuitratorApk.yml
flatpak build-bundle repo guitrator-apk.flatpak io.github.Gu1L3RM3.GuitratorApk
```

## Layout

`domain` (types, `AdbCommandExecutor` trait) → `application` (use cases) → `presentation` (GUI/CLI/MCP), with `infrastructure` (real `adb`, F-Droid download) behind the trait. Tests use stub executors, so no phone is needed.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md), [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) and [SECURITY.md](SECURITY.md). Released under the [MIT license](LICENSE); changes are listed in [CHANGELOG.md](CHANGELOG.md).
