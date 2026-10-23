# Contributing

Thanks for helping! Bug reports, ideas and pull requests are all welcome.

## Setup

Rust 1.77+. On Linux also `libgtk-3-dev libxkbcommon-dev libssl-dev pkg-config`.

```bash
cargo run              # GUI
cargo test
cargo fmt
cargo clippy --all-targets -- -D warnings
```

CI runs the last three on Linux and Windows, so please make them pass locally.

## Code layout

`domain` → `application` (use cases) → `presentation` (GUI, CLI, MCP); `infrastructure` holds the real `adb` runner and the F-Droid download. Put new logic behind `AdbCommandExecutor` so it can be tested with a stub, no phone needed.

## Pull requests

1. Open an issue first for anything large.
2. Keep the change focused and add a test for new logic.
3. Add a line under `Unreleased` in `CHANGELOG.md`.
4. Use clear commit messages (`feat:`, `fix:`, `docs:` …).

By contributing you agree that your work is released under the MIT license.
