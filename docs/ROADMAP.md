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
| Keyboard | Everything reachable from the keyboard: F1 shortcut list, F10 / Alt+letter menus with arrow keys and red hotkey letters, Dependencies dialog keys, Alt+F8/Alt+F7 through messages, Tab/Shift+Tab indent; Turbo-style block cursor; the editor has focus at startup |
| Search | Search menu: Find (Ctrl+F) with all matches highlighted and a match count, Replace / Replace all (Ctrl+H, undoable), Search again (Ctrl+L), Go to line (Ctrl+G) |
| Platforms | Windows 10/11 (GNU toolchain, no Visual Studio), Ubuntu 24.04+, Fedora, RHEL/Rocky/AlmaLinux 9 and 10 |
| Setup help | Options > Dependencies checks every tool, shows per-OS install commands, installs user-level tools, links to [INSTALL.md](INSTALL.md); Linux checks GUI libraries before opening its window |
| Releases | Windows zip and Linux tar.gz with install scripts; CI on 5 systems; tagging `vX.Y.Z` publishes a release |

## Backlog

In suggested order.

### 1. Debugger (Debug menu)
- Debug Adapter Protocol client: codelldb for Rust, debugpy for Python.
- Breakpoints (Ctrl+F8), step over / into (F8 / F7), run to cursor (F4), Watch window, call stack.
- Add the debug adapters to Options > Dependencies and the install scripts.

### 2. Several open files (Window menu)
- Tabs or a window list, switching between files, per-file undo and language-server state.

### 3. TUI front end
- The same IDE in a terminal (ratatui), sharing the core with the GUI.

### 4. Vim mode, more commands
- Line-wise Visual mode (`V`), repeat (`.`), text objects (`ciw`, `di(`), `f`/`t` motions, marks, block cursor.

### 5. New project from templates
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
- **Undo** restores whole-text snapshots, so the view can jump after a large undo.
- **WSL:** software rendering (`--software`) is unstable under WSLg; the default GPU renderer works.
- **Not yet tested with a real keyboard:** Alt+F5 and typing into the Output window, and the Install button in Options > Dependencies. Automated tests send synthetic keys, which Windows handles differently.
- **No LICENSE file yet.** Until one is added, others may view the code but have no clear right to reuse it.

## For contributors

- Build: `cargo run`; tests: `cargo test`; live language-server tests: `cargo test -- --ignored`.
- Packages: `packaging/package-windows.ps1`, `sh packaging/package-linux.sh`.
- Long-running work (builds, tool checks) must run on a worker thread (`platform::blocking`). Blocking the GUI's async executor makes Wayland compositors drop the window.
- iced editor columns are **byte** offsets in the line, not characters. Convert when talking to compilers and language servers.
