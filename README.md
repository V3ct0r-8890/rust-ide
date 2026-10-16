# rust-ide

A Turbo Pascal–style IDE for **Rust** and **Python**, with a GUI (iced) and, later, a TUI front end.

The goal: open a file, press **F5**, and the IDE silently builds/checks it in the background,
jumps to the first error if it fails, or runs the program (console output panel or its own GUI window) if it succeeds.

## Planned architecture

```
core      text buffer, files, project model, commands, keymaps
services  build runner (cargo / python), error parser, LSP client, DAP client
ui        Turbo-style widgets: menu bar, editor windows, dialogs, status line
frontends gui (iced), tui (ratatui) later
```

Back-end engines (run invisibly by the IDE, never typed by the user):

| Job | Rust | Python |
|---|---|---|
| Build / check | `cargo build --message-format=json` | `py_compile` / `ruff` |
| Run | `cargo run` | `python` / `pythonw` |
| Code intelligence | `rust-analyzer` (LSP) | `pyright` / `ruff` (LSP) |
| Debug | `codelldb` (DAP) | `debugpy` (DAP) |

## Milestones

1. Editor window with Turbo-style menu bar and status line (open / save / exit)
2. F5: build + run, error list that jumps to the source line
3. LSP: completion and live diagnostics
4. DAP: breakpoints, F7/F8 stepping, Watch window
5. TUI front end

## Building

```
cargo run
```

Requires the Rust toolchain (rustup) and, on Windows, the MSVC C++ build tools.
