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
| F5 / Ctrl+F9 | Save, compile and run in the Output window (GUI programs open their own window) |
| Alt+F5 | Show / hide the Output window (Turbo Pascal's "User screen"); Esc also hides it |
| Ctrl+F2 | Program reset: stop the running program |
| Ctrl+Space | Code completion (also opens automatically after `.` and `::`) |
| Alt+X | Exit (press twice to discard unsaved changes) |

The Output window shows the program's output and has an input line for programs that read
from the keyboard. *Run > Run in external terminal* opens a separate terminal instead.

Click a line in the Messages panel to jump to that error. While you type, the language server's
errors are shown live: red/magenta text in the editor, the "Messages (live)" panel, and the status bar. `rust-ide path/to/file.rs` opens a file at startup.

## Milestones

1. ~~Editor window with Turbo-style menu bar and status line (open / save / exit)~~ done
2. ~~F5: build + run, error list that jumps to the source line~~ done
3. ~~Syntax highlighting, LSP completion and live diagnostics~~ done
4. DAP: breakpoints, F7/F8 stepping, Watch window
5. TUI front end

## Supported systems

- Windows 10/11 (GNU toolchain, no Visual Studio needed)
- Ubuntu 24.04 LTS and newer
- Fedora, RHEL 9/10, Rocky Linux, AlmaLinux

At startup rust-ide checks for the tools it needs (Rust, a C linker, Python, rust-analyzer,
basedpyright, ...). If something is missing it opens **Options > Dependencies**, which shows
the install command for your system, can install user-level tools itself, and links to the
**[install guide](docs/INSTALL.md)**. On Linux it also checks the GUI libraries before opening
its window and prints the exact `apt` / `dnf` command if any are missing.

## Building

```
cargo run
```

See **[docs/INSTALL.md](docs/INSTALL.md)** for the toolchain and language servers on each system.

Options: `rust-ide --software` (or `renderer=software` in the settings file) uses software
rendering on machines with broken GPU drivers. Settings live in `%APPDATA%\rust-ide\settings.txt`
or `~/.config/rust-ide/settings.txt`.

Tests: `cargo test`; live language-server tests: `cargo test -- --ignored`.
CI builds and tests on Ubuntu 24.04, Fedora, AlmaLinux 9/10 and Windows.
