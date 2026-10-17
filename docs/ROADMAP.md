# Status and roadmap

Last updated: 2026-10-17. Latest release: [v0.1.0](https://github.com/V3ct0r-8890/rust-ide/releases/tag/v0.1.0).

## Done

| Area | What works |
|---|---|
| Editor | Turbo Pascal-style GUI (iced): menu bar, blue editor, status line, File menu, F2/F3/Alt+X, unsaved-changes guard |
| Build and run | F9 compile/check, F5 build and run for Rust (cargo or single-file rustc) and Python; Messages panel that jumps to errors |
| Output window | Programs run inside the IDE (Alt+F5 show/hide, Esc hides, Ctrl+F2 stops), with an input line for programs that read the keyboard; *Run in external terminal* as an alternative |
| Highlighting | Rust and Python, multi-line comments and strings |
| Language servers | rust-analyzer and basedpyright/pyright: live errors in the editor and a live Messages panel, completion popup (Ctrl+Space, auto after `.` and `::`) |
| Editing | Undo/redo (Ctrl+Z, Ctrl+Y); basic Vim mode (Options > Vim mode) |
| Debugger | Debug menu with Turbo Pascal keys: Ctrl+F8 breakpoint, F7 trace into, F8 step over, Shift+F8 step out, F4 go to cursor, Shift+F5 debug run, Ctrl+F7 watch, Ctrl+F2 reset. Rust through `gdb -i dap` (gdb 14+), Python through debugpy. Breakpoints and the execution line are colored in the editor; the debug panel shows watches, arguments and locals, and the call stack |
| Terminal version | `rust-ide-tui` (ratatui + crossterm): Turbo colors, block cursor, menus with hotkeys, F1 help, windows, find / search again / go to line, F9 compile with messages, F5 runs the program in the terminal (user screen, Alt+F5), open-file dialog. Shares the build runner, search and lexers with the GUI through the library crate |
| Windows | Several open files: tabs under the menu bar, F6 / Shift+F6, Alt+1-9, Window menu (Alt+0) with the window list, Alt+F3 close; each window has its own text, undo and diagnostics; F3, error jumps and the debugger open files in windows; `rust-ide a.rs b.py` opens several files |
| Keyboard | Everything reachable from the keyboard: F1 shortcut list, F10 / Alt+letter menus with arrow keys and red hotkey letters, Dependencies dialog keys, Alt+F8/Alt+F7 through messages, Tab/Shift+Tab indent; Turbo-style block cursor; the editor has focus at startup |
| Search | Search menu: Find (Ctrl+F) with all matches highlighted and a match count, Replace / Replace all (Ctrl+H, undoable), Search again (Ctrl+L), Go to line (Ctrl+G) |
| Platforms | Windows 10/11 (GNU toolchain, no Visual Studio), Ubuntu 24.04+, Fedora, RHEL/Rocky/AlmaLinux 9 and 10 |
| Setup help | Options > Dependencies checks every tool, shows per-OS install commands, installs user-level tools, links to [INSTALL.md](INSTALL.md); Linux checks GUI libraries before opening its window |
| Releases | Windows zip and Linux tar.gz with install scripts; CI on 5 systems; tagging `vX.Y.Z` publishes a release |

## Backlog

In suggested order.

### 1. Terminal version: the rest
- Live errors and completion (language servers), the debugger, Replace and Vim mode in `rust-ide-tui`.
- Mouse support (click to place the cursor, menus).

### 2. Vim mode, more commands
- Line-wise Visual mode (`V`), repeat (`.`), text objects (`ciw`, `di(`), `f`/`t` motions, marks, block cursor.

### 3. New project from templates
- **File > New project...** dialog: choose the language (Rust or Python), a size
  (Simple, Low, Medium or Complex), a project name and a folder. The IDE generates a general
  starting template and opens its main file.
- Proposed template contents (to be confirmed before implementing):

| Size | Rust | Python |
|---|---|---|
| Simple | Cargo project with one `main.rs` (hello world) | One script `main.py` |
| Low | Binary crate split into a few modules, with unit tests | Package folder with `__main__.py`, a module and pytest tests |
| Medium | Library + binary crate, error handling, config file, integration tests, README | `pyproject.toml`, `src/` layout, `.venv` setup, pytest, lint settings, README |
| Complex | Cargo workspace (core library, CLI, extra crates), tests, benchmarks, CI workflow | Multi-module `src/` package with CLI entry point, config, logging, tests, CI workflow |

- Templates should build and run with F5 straight away, and pass their own tests.

## Known limitations

- **Completion popup** sits in the bottom-right corner of the editor, not next to the cursor: the editor widget does not report the cursor's screen position.
- **Output window** is not a full terminal: programs see pipes, not a TTY, so colors and full-screen terminal programs do not work. ANSI codes are stripped.
- **Block cursor** is drawn only while the IDE window has focus (it follows the editor's input-method caret).
- **Debugging programs that read the keyboard** is not supported yet: the debugger has no input line. Use F5 for those.
- **Breakpoints stay on their line numbers** when lines are added or removed above them.
- **Live errors** are shown for the active window; a background window gets fresh ones when you switch to it.
- **Undo** restores whole-text snapshots, so the view can jump after a large undo.
- **WSL:** software rendering (`--software`) is unstable under WSLg; the default GPU renderer works.
- **Not yet tested with a real keyboard:** Alt+F5 and typing into the Output window, and the Install button in Options > Dependencies. Automated tests send synthetic keys, which Windows handles differently.
- **No LICENSE file yet.** Until one is added, others may view the code but have no clear right to reuse it.

## For contributors

- Build: `cargo run`; tests: `cargo test`; live language-server tests: `cargo test -- --ignored`.
- Packages: `packaging/package-windows.ps1`, `sh packaging/package-linux.sh`.
- Long-running work (builds, tool checks) must run on a worker thread (`platform::blocking`). Blocking the GUI's async executor makes Wayland compositors drop the window.
- iced editor columns are **byte** offsets in the line, not characters. Convert when talking to compilers and language servers.
