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

Everything works from the keyboard. **F1 lists every shortcut.**

| Key | Action |
|---|---|
| F1 | Keyboard shortcut list |
| F10 / Alt+letter | Open a menu (Alt+F File, Alt+E Edit, Alt+S Search, Alt+R Run, Alt+C Compile, Alt+O Options, Alt+H Help); arrows and Enter, or the red letter, choose an item |
| F2 / Ctrl+S | Save (asks for a name the first time) |
| F3 / Ctrl+O | Open (each file in its own window) |
| F9 | Compile / syntax-check, list errors, jump to the first one |
| F5 / Ctrl+F9 | Save, compile and run in the Output window (GUI programs open their own window) |
| Alt+F5 | Show / hide the Output window (Turbo Pascal's "User screen"); Esc also hides it |
| Ctrl+F2 | Program reset: stop the running program |
| Ctrl+Space | Code completion (also opens automatically after `.` and `::`) |
| Ctrl+Z / Ctrl+Y | Undo / redo (Ctrl+Shift+Z also redoes) |
| Ctrl+F / Ctrl+H | Find / Replace bar (all matches highlighted, "Match 2 of 7" in the status bar) |
| Ctrl+L | Search again (Turbo Pascal's key), even with the bar closed |
| Ctrl+G | Go to line number |
| Alt+M | Match case on/off in the search bar (Tab moves between its fields) |
| Alt+F8 / Alt+F7 | Next / previous error in the Messages panel |
| Tab / Shift+Tab | Indent / unindent |
| Ctrl+F8 | Toggle breakpoint on the cursor line |
| F7 / F8 | Debugger: trace into / step over (starts at the first line); Shift+F8 steps out |
| F4 | Go to cursor (run to the cursor line) |
| Shift+F5 | Debug run to the next breakpoint; F5 / Ctrl+F9 continue while debugging |
| Ctrl+F7 | Add a watch expression |
| F6 / Shift+F6 | Next / previous window (open files are shown as tabs) |
| Alt+1 ... Alt+9, Alt+0 | Go to window 1-9; Alt+0 lists the windows |
| Alt+F3 | Close the window |
| Alt+X | Exit (press twice to discard unsaved changes) |

The Output window shows the program's output and has an input line for programs that read
from the keyboard. *Run > Run in external terminal* opens a separate terminal instead.

Click a line in the Messages panel to jump to that error. While you type, the language server's
errors are shown live: red/magenta text in the editor, the "Messages (live)" panel, and the status bar. `rust-ide a.rs b.py` opens files at startup, each in its own window.

## Vim mode

Turn it on in **Options > Vim mode** (remembered between sessions). The status bar shows
`NORMAL`, `-- INSERT --` or `-- VISUAL --` and any pending command. Supported:

- Modes: `i a I A o O s S` to insert, `Esc` back to Normal, `v` for Visual
- Motions (with counts): `h j k l w b e 0 ^ $ gg G`, `Ctrl+D`/`Ctrl+U`
- Operators with motions: `d c y > <` (e.g. `dw`, `c$`, `y2j`, `dG`), doubled for lines (`dd`, `3yy`, `>>`)
- Editing: `x X D C Y p P J r<char> u Ctrl+R`
- Visual: motions, then `d x y c > <`
- Search: `/text` then `n` / `N`
- Commands: `:w`, `:q`, `:q!`, `:wq`, `:x`, `:<line>`

The function keys (F2, F5, F9, Alt+F5, ...) keep working in every mode.

## Download

Prebuilt packages are on the [Releases page](https://github.com/V3ct0r-8890/rust-ide/releases):

- **Windows 10/11**: `rust-ide-<version>-windows-x86_64.zip`. Unzip and run `rust-ide.exe`;
  `install-deps.ps1` installs Rust (GNU toolchain), MinGW, Python and the language servers.
- **Linux x86_64** (Ubuntu 24.04+, Fedora, RHEL/Rocky/AlmaLinux 9 and 10):
  `rust-ide-<version>-linux-x86_64.tar.gz`. Unpack, run `./install-deps.sh` for the tools
  and `./install.sh` to add `rust-ide` to `~/.local/bin` and your application menu.

To build the packages yourself: `packaging/package-windows.ps1` or `sh packaging/package-linux.sh`
(output in `dist/`).

## Status

Done: several open files (tabs, F6, Window menu), debugger (breakpoints, F7/F8 stepping, watches, locals, call stack), editor with a Turbo-style block cursor and full keyboard control (F1 help, F10 menus), build and run (F9/F5) with the in-IDE Output window, highlighting, live errors
and completion, undo, basic Vim mode, Search menu (find, replace, go to line),
Windows and Linux packages.

Next: TUI front end,
new-project templates (Simple / Low / Medium / Complex for Rust and Python).

See **[docs/ROADMAP.md](docs/ROADMAP.md)** for the full backlog and known limitations.

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
