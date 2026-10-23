# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/) and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Refresh button (and F5) that reloads phone info and the app list while keeping the old list visible.
- App details screen: version, API levels, dates, installer, size, memory and permissions.
- Per-app icons, read straight from the APK on the phone without copying it to the computer.
- Phone summary (model, Android version, RAM, storage), plus APK size and memory use per app.
- CLI: `list --detailed`, `list --user-only`, `device` and `info`.
- MCP tools: `list_apps`, `device_info`, `app_details` and `download_apk`.

### Fixed
- MCP `tools/call` ignored the tool name and treated every call as `extract_apk`; it now dispatches by name.
- Clearer errors for an uninstalled package and an invalid package name.

## [1.0.0] - 2026-10-23

### Added
- GUI with Phone and F-Droid tabs, search, system-app filter, status bar and app icon.
- CLI commands `list`, `pull` and `download` (F-Droid, optional `--install`).
- MCP server (`mcp`) and `setup-mcp claude|codex` to register it.
- Prebuilt binaries for Linux and Windows, and a Flatpak bundle, attached to each release.
- Automatic download of Android platform-tools when `adb` is not installed.
