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

## Keys

| Key | Action |
|---|---|
| F2 / Ctrl+S | Save (asks for a name the first time) |
| F3 / Ctrl+O | Open |
| F9 | Compile / syntax-check, list errors, jump to the first one |
| F5 / Ctrl+F9 | Save, compile and run (console window, or the program's own GUI window) |
| Alt+X | Exit (press twice to discard unsaved changes) |

Click a line in the Messages panel to jump to that error. `rust-ide path/to/file.rs` opens a file at startup.

## Milestones

1. ~~Editor window with Turbo-style menu bar and status line (open / save / exit)~~ done
2. ~~F5: build + run, error list that jumps to the source line~~ done
3. LSP: completion and live diagnostics
4. DAP: breakpoints, F7/F8 stepping, Watch window
5. TUI front end

## Building

```
cargo run
```

Requires the Rust toolchain via rustup. On Windows this project uses the open-source GNU toolchain
(`rustup default stable-x86_64-pc-windows-gnu`) plus MinGW-w64 binutils for `dlltool`
(e.g. `winget install BrechtSanders.WinLibs.POSIX.MSVCRT`), so no Visual Studio is needed.
Python 3 on `PATH` is needed for Python files.
