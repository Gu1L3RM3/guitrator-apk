# Guitrator APK Extractor

> A cross-platform desktop tool to download installed APKs from Android devices connected via USB — with a GUI, a CLI, and an MCP server, all sharing the same core logic.

![Rust](https://img.shields.io/badge/Rust-1.77%2B-orange?logo=rust)
![License](https://img.shields.io/badge/license-MIT-blue)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Windows-lightgrey)

---

## Features

| Interface | Description |
|-----------|-------------|
| **GUI** | Native desktop window (`egui`/`eframe`) with search, scrollable app list, and folder picker |
| **CLI** | Terminal commands (`list`, `pull`) via `clap` |
| **MCP Server** | JSON-RPC stdio server for AI assistants (Cursor, Claude, etc.) |

- **ADB auto-download** — if `adb` is not found in `PATH`, platform-tools are downloaded automatically from Google.
- **No device required for testing** — core logic is fully unit-tested with mock executors.
- **Clean architecture** — Domain → Application → Presentation; no layer bleeds into another.

---

## Requirements

- Rust 1.77+ (`rustup.rs`)
- An Android device with **USB Debugging enabled**
- USB cable

> **ADB is optional at install time.** If `adb` is not on your `PATH`, the app will download Android Platform Tools automatically on first run.

---

## Installation

```bash
git clone https://github.com/yourname/guitrator-apk
cd guitrator-apk
cargo build --release
```

The binary will be at `target/release/guitrator-apk`.

---

## Usage

### GUI (default)

```bash
./guitrator-apk
# or explicitly:
./guitrator-apk gui
```

1. Connect your Android device via USB with debugging enabled.
2. Click **🔄 Refresh** to list installed packages.
3. Use the search bar to filter by package name.
4. Click **⬇ Extract** next to any app.
5. Choose the destination folder with **Browse…**.

### CLI

**List all installed packages:**

```bash
./guitrator-apk list
```

```
PACKAGE                                                      APK PATH
----------------------------------------------------------------------------------------------------
com.android.chrome                                           /data/app/com.android.chrome-abc/base.apk
com.example.myapp                                            /data/app/com.example.myapp-xyz/base.apk

2 package(s) found.
```

**Pull a specific APK:**

```bash
./guitrator-apk pull --package com.example.myapp \
                     --apk-path /data/app/com.example.myapp-xyz/base.apk \
                     --destination ~/Downloads
```

```
Saved: /home/user/Downloads/com.example.myapp.apk
```

### MCP Server (AI integration)

Start the server:

```bash
./guitrator-apk mcp
```

The server communicates over **stdin/stdout** using JSON-RPC 2.0. Logs go to **stderr** only, so stdout remains clean.

#### Add to Cursor / Claude Desktop

**Cursor** (`~/.cursor/mcp.json`):

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

**Claude Desktop** (`claude_desktop_config.json`):

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

#### Available MCP Resources

| URI | Description |
|-----|-------------|
| `adb://device/installed_apps` | Plain-text list of all installed packages and their device APK paths |

#### Available MCP Tools

| Tool | Arguments | Description |
|------|-----------|-------------|
| `extract_apk` | `package_name` *(required)*, `apk_path` *(required)*, `destination` *(optional, default `.`)* | Pulls the APK from the device to the local filesystem |

---

## Project Structure

```
src/
├── main.rs                        # Entry point — routes to GUI, CLI, or MCP
├── lib.rs                         # Re-exports for integration tests
│
├── domain/
│   ├── mod.rs
│   └── app_info.rs                # AppInfo struct · AdbCommandExecutor trait · AdbError · parser
│
├── infrastructure/
│   ├── mod.rs
│   ├── adb_executor.rs            # RealAdbRunner — spawns the actual adb process
│   └── adb_downloader.rs          # ensure_adb() — PATH check → cache → auto-download
│
├── application/
│   ├── mod.rs
│   ├── list_apps.rs               # ListApps use case
│   └── extract_apk.rs             # ExtractApk use case
│
├── presentation/
│   ├── mod.rs
│   ├── cli.rs                     # clap CLI (list / pull subcommands)
│   ├── gui.rs                     # egui UI with non-blocking background threads
│   └── mcp.rs                     # JSON-RPC 2.0 stdio MCP server
│
└── tests/
    └── integration_test.rs        # End-to-end tests with stub executor (no device needed)
```

### Architecture Diagram

```
┌─────────────────────────────────────────┐
│               Presentation              │
│   ┌───────┐  ┌───────┐  ┌───────────┐  │
│   │  GUI  │  │  CLI  │  │ MCP Server│  │
│   └───┬───┘  └───┬───┘  └─────┬─────┘  │
└───────┼──────────┼────────────┼─────────┘
        │          │            │
┌───────▼──────────▼────────────▼─────────┐
│              Application                │
│        ListApps · ExtractApk            │
└───────────────────┬─────────────────────┘
                    │
┌───────────────────▼─────────────────────┐
│                 Domain                  │
│   AppInfo · AdbCommandExecutor trait    │
│         AdbError · parse_package_list   │
└───────────────────┬─────────────────────┘
                    │
┌───────────────────▼─────────────────────┐
│             Infrastructure              │
│   RealAdbRunner · ensure_adb()          │
└─────────────────────────────────────────┘
```

---

## Running Tests

Tests do **not** require a physical device — the executor is mocked.

```bash
cargo test
```

Expected output:

```
running 7 tests
test domain::app_info::tests::parse_three_packages ... ok
test domain::app_info::tests::parse_empty_returns_empty_vec ... ok
test domain::app_info::tests::parse_ignores_blank_lines ... ok
test domain::app_info::tests::parse_missing_prefix_returns_error ... ok
test domain::app_info::tests::parse_missing_equals_returns_error ... ok
test application::list_apps::tests::parses_executor_output ... ok
test application::list_apps::tests::propagates_no_device_error ... ok
test application::extract_apk::tests::calls_adb_pull_with_correct_args ... ok
test application::extract_apk::tests::returns_error_on_adb_failure ... ok
test tests::integration_test::end_to_end_list_and_extract ... ok
```

---

## Design Principles

### Clean Architecture / SOLID

- **SRP** — Each module has one reason to change. `adb_downloader` handles binary acquisition; `adb_executor` handles process spawning; `gui` handles rendering.
- **OCP** — New interfaces (e.g. REST API) can be added without touching domain or application layers.
- **DIP** — GUI and CLI depend on the `AdbCommandExecutor` *trait*, not on `RealAdbRunner` directly. Swap implementations freely.

### TDD

All domain parsing logic and use cases were written test-first. The `AdbCommandExecutor` trait exists specifically to allow fast, offline unit tests without a real device.

### Error Handling

No `unwrap()` in production code. All errors are typed via `thiserror` and propagate through `Result<_, AdbError>`.

### MCP stdout cleanliness

`env_logger` is configured to write exclusively to **stderr**. The MCP server can therefore use **stdout** purely for JSON-RPC frames without interference.

---

## Platform Support

| OS | Status |
|----|--------|
| Linux | ✅ Tested (Pop!_OS) |
| Windows | ✅ Supported (cross-compiled; ADB auto-download uses the Windows zip) |
| macOS | ⚠️ Should work, not officially tested |

---

## License

MIT — see [LICENSE](LICENSE).
