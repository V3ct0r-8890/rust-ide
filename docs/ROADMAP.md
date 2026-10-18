# Status and roadmap

Last updated: 2026-10-19. Latest release: [v0.2.0](https://github.com/V3ct0r-8890/rust-ide/releases/tag/v0.2.0).

## Done

| Area | What works |
|---|---|
| Editor | Turbo Pascal-style GUI (iced): menu bar, blue editor, status line, File menu, F2/F3/Alt+X, unsaved-changes guard |
| Build and run | F9 compile/check, F5 build and run for Rust (cargo or single-file rustc) and Python; Messages panel that jumps to errors |
| Output window | Programs run inside the IDE (Alt+F5 show/hide, Esc hides, Ctrl+F2 stops), with an input line for programs that read the keyboard; *Run in external terminal* as an alternative |
| Highlighting | Rust and Python, multi-line comments and strings |
| Language servers | rust-analyzer and basedpyright/pyright: live errors in the editor and a live Messages panel, completion popup (Ctrl+Space, auto after `.` and `::`) |
| Editing | Undo/redo (Ctrl+Z, Ctrl+Y); basic Vim mode (Options > Vim mode) |
| Window style | Options > Window style (window version): **Legacy windows** (Turbo Pascal frame line with the name and window number), **Horizontal tabs** (default) or **Vertical tabs** (list on the left); same keys in every style; the choice is remembered |
| New files | Started without a file, the IDE (both versions) asks which language to use: Rust, Python, or Text / Markdown for anything else; File > New asks too. The last choice is remembered and preselected; the type sets highlighting, the `NONAME.xx` name and the Save As default; `.txt` and `.md` open as plain files |
| Syntax check | Compile > Syntax check, **Alt+F9** (GUI and terminal version): finds errors without building or running - `cargo check` for projects, `rustc --emit=metadata` for single Rust files, the syntax parse for Python; errors go to the Messages panel and the cursor jumps to the first. Options > Check before run (GUI) makes F9 / F5 run it first and stop on errors |
| Debugger | Debug menu with Turbo Pascal keys: Ctrl+F8 breakpoint, F7 trace into, F8 step over, Shift+F8 step out, F4 go to cursor, Shift+F5 debug run, Ctrl+F7 watch, Ctrl+F2 reset. Rust through `gdb -i dap` (gdb 14+), Python through debugpy. Breakpoints and the execution line are colored in the editor; the debug panel shows watches, arguments and locals, and the call stack |
| Terminal version | `rust-ide-tui` (ratatui + crossterm): Turbo colors, block cursor, menus with hotkeys, F1 help, windows, find / search again / go to line, F9 compile with messages, F5 runs the program in the terminal (user screen, Alt+F5), open-file dialog. Shares the build runner, search and lexers with the GUI through the library crate |
| Windows | Several open files: tabs under the menu bar, F6 / Shift+F6, Alt+1-9, Window menu (Alt+0) with the window list, Alt+F3 close; each window has its own text, undo and diagnostics; F3, error jumps and the debugger open files in windows; `rust-ide a.rs b.py` opens several files |
| Keyboard | Everything reachable from the keyboard: F1 shortcut list, F10 / Alt+letter menus with arrow keys and red hotkey letters, Dependencies dialog keys, Alt+F8/Alt+F7 through messages, Tab/Shift+Tab indent; Turbo-style block cursor; the editor has focus at startup |
| Search | Search menu: Find (Ctrl+F) with all matches highlighted and a match count, Replace / Replace all (Ctrl+H, undoable), Search again (Ctrl+L), Go to line (Ctrl+G) |
| Platforms | Windows 10/11 (GNU toolchain, no Visual Studio), Ubuntu 24.04+, Fedora, RHEL/Rocky/AlmaLinux 9 and 10 |
| Setup help | Options > Dependencies checks every tool, shows per-OS install commands, installs user-level tools, links to [INSTALL.md](INSTALL.md); Linux checks GUI libraries before opening its window |
| Releases | Windows zip and Linux tar.gz with install scripts; CI on 5 systems; tagging `vX.Y.Z` publishes a release |

## Backlog

Ordered from easiest to hardest. Effort: **S** = small, **M** = medium, **L** = large.

| # | Item | Effort |
|---|---|---|
| 1 | Markdown preview (window version first) | S-M |
| 2 | Font size setting | M |
| 3 | Vim mode, more commands | M |
| 4 | New project from templates | M |
| 5 | Terminal version: the rest | L |
| 6 | C# support | L |

### 1. Markdown preview - S-M
- Today `.md` files open as plain text: no highlighting and no preview. This adds a **preview**
  that shows the rendered document next to the editor and follows what you type.
- **Window version first:** iced has a Markdown widget (an optional feature of the library we
  already use), so headings, bold / italic, lists, quotes, code blocks and links come ready-made;
  clicking a link opens the browser. To check while building: tables and images.
- Proposed: side by side (editor left, preview right), switched on and off with one key (Alt+F6
  proposed, also in a menu) and scrolled with the keyboard. Whether it should also be a
  full-window preview is decided when designing it.
- Only offered for `.md` windows (and untitled ones of the Markdown type).
- **Terminal version later:** a small renderer built on the `pulldown-cmark` parser (headings,
  bold, lists, code blocks as styled text), part of "Terminal version: the rest".
- Optional: light highlighting in the editor for Markdown source (headings, `**bold**`, code).

### 2. Font size setting - M
- **Options > Font size...** in the GUI, plus keys: Ctrl+= larger, Ctrl+- smaller, Ctrl+0 back
  to the default. Applies to the editor, menus, panels and dialogs together.
- Remembered between sessions in the settings file (`font_size=16`), with a sensible range
  (for example 10 to 32).
- Why medium: iced fixes its default text size when the program starts, so a live change means
  giving every piece of text (and the block cursor's measurements) one shared size value.
- The terminal version uses the terminal's own font; its Options menu can say so and point to
  the terminal's zoom (usually Ctrl+= / Ctrl+-).

### 3. Vim mode, more commands - M
- Line-wise Visual mode (`V`), repeat (`.`), text objects (`ciw`, `di(`), `f`/`t` motions, marks,
  block cursor.
- Pure logic in `src/vim.rs`, so it is tested with unit tests like the existing commands.

### 4. New project from templates - M
- **File > New project...** dialog: choose the language (Rust or Python), a size
  (Simple, Low, Medium or Complex), a project name and a folder. The IDE generates a general
  starting template and opens its main file.
- Template contents below are proposals; **each size is confirmed with you during design,
  before it is built.**

| Size | Rust | Python |
|---|---|---|
| Simple | Cargo project with one `main.rs` (hello world) | One script `main.py` |
| Low | Binary crate split into a few modules, with unit tests | Package folder with `__main__.py`, a module and pytest tests |
| Medium | Library + binary crate, error handling, config file, integration tests, README | `pyproject.toml`, `src/` layout, `.venv` setup, pytest, lint settings, README |
| Complex | Cargo workspace (core library, CLI, extra crates), tests, benchmarks, CI workflow | Multi-module `src/` package with CLI entry point, config, logging, tests, CI workflow |

- Templates should build and run with F5 straight away, and pass their own tests; all eight
  (4 sizes x 2 languages) are checked by automated tests and in CI.

### 5. Terminal version: the rest - L
Suggested sub-steps, easiest first:
1. Replace, mouse support (click to place the cursor, menus) and an Options menu (Check before run, window style, ...).
2. Vim mode (reuse `src/vim.rs`).
3. Live errors and completion (language servers). Needs the terminal loop to wait for events
   from the servers as well as keys, which is the main rework.
4. The debugger (breakpoints, F7/F8, watches panel).

### 6. C# support (open-source tools only) - L
- **Compiler:** the open-source .NET SDK command line (MIT licensed, Roslyn compiler), current
  LTS release. F9 runs `dotnet build`, F5 runs `dotnet run`; errors come from the build output
  (`file(line,col): error CS0103: ...`) into the Messages panel. A single `.cs` file without a
  project runs with `dotnet run file.cs` (file-based apps, .NET 10+).
- **Live errors and completion:** an open-source C# language server, `csharp-ls` (MIT, installed
  with `dotnet tool install -g csharp-ls`).
- **Debugger:** `netcoredbg` (Samsung, MIT), which speaks the Debug Adapter Protocol like gdb and
  debugpy. Microsoft's own debugger (`vsdbg`) is not open source and is not used.
- **Highlighting:** a C# lexer (keywords, strings including `@"..."` and `$"..."`, comments,
  attributes).
- **Setup:** add the .NET SDK, csharp-ls and netcoredbg to Options > Dependencies, the install
  scripts and docs/INSTALL.md: Windows `winget install Microsoft.DotNet.SDK.10`, Ubuntu
  `sudo apt install dotnet-sdk-10.0` (or Microsoft's package feed), Fedora/RHEL
  `sudo dnf install dotnet-sdk-10.0`; netcoredbg from its GitHub releases.
- **Templates:** add C# to File > New project (`dotnet new console`, class library + tests,
  solution with several projects), once item 4 exists.
- Why last: it adds a whole new toolchain on three systems, and netcoredbg is unproven here.
  Check before starting: the exact SDK and package names for each system, and whether
  netcoredbg covers the needed features (breakpoints, stepping, locals) on Windows and Linux.

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
