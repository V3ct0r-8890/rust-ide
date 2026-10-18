# Status and roadmap

Last updated: 2026-10-20. Latest release: [v0.2.0](https://github.com/V3ct0r-8890/rust-ide/releases/tag/v0.2.0).

## Done

| Area | What works |
|---|---|
| Editor | Turbo Pascal-style GUI (iced): menu bar, blue editor, status line, File menu, F2/F3/Alt+X, unsaved-changes guard |
| Build and run | F9 compile/check, F5 build and run for Rust (cargo or single-file rustc) and Python; Messages panel that jumps to errors |
| Output window | Programs run inside the IDE (Alt+F5 show/hide, Esc hides, Ctrl+F2 stops), with an input line for programs that read the keyboard; *Run in external terminal* as an alternative |
| Highlighting | Rust and Python, multi-line comments and strings |
| Language servers | rust-analyzer and basedpyright/pyright: live errors in the editor and a live Messages panel, completion popup (Ctrl+Space, auto after `.` and `::`) |
| Editing | Undo/redo (Ctrl+Z, Ctrl+Y); Vim mode (Options > Vim mode): motions, operators, text objects (`ciw`, `di(`, `da"`), `f`/`t` with `;` `,`, `.` repeat, marks, Visual and Visual-line mode |
| Markdown preview | Window version: **Alt+F6** / Alt+P / Window menu shows the rendered `.md` next to the editor (iced's Markdown widget: headings, emphasis, lists, quotes, tables, code blocks, links); follows typing and the cursor; links open in the browser or as files. **Options > Preview in** (Alt+O P) picks where it shows: side by side, in its own tab (refreshed when you select it), or in a separate window (follows typing) |
| Window style | Options > Window style (window version): **Legacy windows** (Turbo Pascal frame line with the name and window number), **Horizontal tabs** (default) or **Vertical tabs** (list on the left); same keys in every style; the choice is remembered |
| New files | Started without a file, the IDE (both versions) asks which language to use: Rust, Python, or Text / Markdown for anything else; File > New asks too. The last choice is remembered and preselected; the type sets highlighting, the `NONAME.xx` name and the Save As default; `.txt` and `.md` open as plain files |
| New project | **File > New project...** (Alt+F P, both versions): Rust or Python, size Simple / Low / Medium / Complex, a name and a folder; writes the project and opens its main file. All 8 templates are built, run and tested by `cargo test templates:: -- --ignored` (run in CI) |
| Terminal: Replace, mouse, Options | `rust-ide-tui` (0.2.12): Replace all (Ctrl+H / Ctrl+R, one undo), mouse (menus, tabs, cursor, wheel; `--no-mouse` turns it off), Options > Check before run, Options > Line numbers (0.2.13) |
| Settings dialog | Window version: **Options > Settings...** (Alt+O E) - preview place, window style, Vim mode, check before run, font size, dependencies; keyboard (Up/Down, Left/Right, Enter, letters, Esc) and mouse; changes apply and are remembered at once. The terminal version has no Options menu yet (backlog item 3) |
| Font size | Window version: **Ctrl+=** larger, **Ctrl+-** smaller, **Ctrl+0** default (also Options menu, Alt+O B / S / R); 10 to 32 points, remembered (`font_size` in the settings file). Applies to the editor (with the block cursor), the Markdown preview and the user screen; menus and panels keep their size. The terminal version uses the terminal's own zoom |
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
| 1 | Development environments for project sessions (**next**) | L |
| 2 | Line numbers in the window version | M |
| 3 | Terminal version: the rest | L |
| 4 | C# support | L |

### 1. Development environments for project sessions - L (do this next)
Set up and remember a custom environment per project, so a project always builds and runs with
the tools and settings chosen for it, without touching the machine-wide setup. **The details
below are proposals; each part is confirmed with you before it is built.**
- **Two groups of environment settings**, kept apart:
  1. **General (system-wide) environment**, for every project and session of the IDE: proxy
     settings (`HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY`, cargo / pip proxy and certificate
     settings), extra `PATH` entries, tool locations (`CARGO_HOME`, `RUSTUP_HOME`), mirrors and
     registries. Edited in Options > Environment... and saved in the IDE's settings file. Applied
     to everything the IDE starts: builds, runs, the debugger, language servers, installs and the
     Dependencies checks.
  2. **Project environment**, for one project folder: variables for the program (e.g. `DATABASE_URL`,
     `RUST_LOG`), the Python interpreter / `.venv`, the Rust toolchain, working directory and run
     arguments. Stored with the project (e.g. `.rust-ide/env.toml`, or a `.env` file the IDE
     reads), so it can be committed or ignored per project. A project value overrides a general
     one with the same name. The `.venv` holds the project's Python and packages; variables are
     applied by the IDE itself when it starts the program (a venv's `activate` script only affects
     shells), so they also work for Rust projects.
  The dialog shows both groups and the final result for the current project (which value wins and
  where it came from). Secrets (passwords, tokens) are never committed by the IDE: the project file
  is added to `.gitignore` when it holds variables.
- **Python:** create and select a `.venv` per project (Options > Environment...), install the
  project's requirements (`pip install -r` / `pip install -e ".[dev]"`), show the interpreter in
  the status line, and use it for F5, pytest, the debugger and the language server.
- **Rust:** choose the toolchain per project (`rust-toolchain.toml`, or the IDE's own setting) and
  the target; show it in the status line.
- **Managed tools (answer to "can we bundle Rust and Python?"):** do not put them inside the
  installer (hundreds of MB, licence notices, always out of date). Instead, offer **"Set up
  tools for the IDE"**, which downloads them on first use into the IDE's own folder, separate from
  the system: Rust through `rustup` with `RUSTUP_HOME` / `CARGO_HOME` under the IDE's data folder
  (minimal profile, about 300 MB; the Windows GNU target still needs a MinGW `gcc`, which would be
  fetched too), and Python as a relocatable build (python-build-standalone, which includes `pip`
  and `venv`; the python.org embeddable zip has neither). The Dependencies dialog already installs
  user-level tools and would be extended to do this.
- **Sessions:** the open windows, cursor positions, window style and the project's environment are
  saved when the IDE closes and restored next time (per project folder).
- **Both front ends** read the same environment file; the window version gets the dialog first.
- To confirm: the format and place of the project file (`.rust-ide/env.toml` or `.env`), whether
  general variables may also be set per user vs per machine, and whether the managed download
  should be opt-in.

### 2. Line numbers in the window version - M
- An **Options > Line numbers** toggle (also in the Settings dialog), remembered, like the one the
  terminal version has since 0.2.13.
- The difficulty: iced 0.14's `text_editor` shows no gutter and reports no scroll position, so a
  separate column of numbers cannot be kept in step with the text. Options to decide between:
  1. A custom editor widget (own drawing, scrolling, cursor, selection) - the most work, but it
     would also allow breakpoint dots and error marks in the gutter.
  2. Patching or forking `text_editor` to expose its scroll offset, then drawing the numbers in a
     column beside it.
  3. Only with wrapping off and the editor taller than the file: a plain column, hidden once the
     text scrolls (cheap but incomplete; not recommended).
- Must handle: files with Thai text (byte offsets), the Markdown preview next to the editor,
  every window style, font size changes, and line numbers wider than the column when the file
  grows past 999 lines.
- Confirm the option with you before building.

### 3. Terminal version: the rest - L
Suggested sub-steps, easiest first:
1. ~~Replace, mouse support and an Options menu~~ - done in 0.2.12 (Ctrl+H / Ctrl+R replace all, click menus / tabs / text, Options > Check before run). Still to do: a Settings dialog like the window version's, once there are more settings to put in it (Vim, ...).
2. Vim mode (reuse `src/vim.rs`).
3. Live errors and completion (language servers). Needs the terminal loop to wait for events
   from the servers as well as keys, which is the main rework.
4. The debugger (breakpoints, F7/F8, watches panel).
5. Markdown preview: a small renderer built on the `pulldown-cmark` parser (headings, bold, lists, code blocks as styled text).

### 4. C# support (open-source tools only) - L
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
  solution with several projects), once item 1 exists.
- Why last: it adds a whole new toolchain on three systems, and netcoredbg is unproven here.
  Check before starting: the exact SDK and package names for each system, and whether
  netcoredbg covers the needed features (breakpoints, stepping, locals) on Windows and Linux.

## Known limitations

- **Line numbers** exist only in the terminal version; iced's text editor exposes no scroll position (backlog item 2).

- **Markdown preview** shows images only as their description, does not follow `#heading` links, and does not color code blocks.
- **Completion popup** sits in the bottom-right corner of the editor, not next to the cursor: the editor widget does not report the cursor's screen position.
- **Output window** is not a full terminal: programs see pipes, not a TTY, so colors and full-screen terminal programs do not work. ANSI codes are stripped.
- **Vim mode** `w` and `b` use the editor's word motion (`w` stops at the end of a word, like `e`); `.` does not repeat Visual-mode changes or take a count; marks do not follow edits and are shared by all windows; tag objects (`it`, `at`) and `p` in Visual mode are missing; the Visual-line highlight stops at the cursor column on the cursor's line.
- **Preview tab** is not live: it re-reads the file when you select the tab. The preview window follows the active `.md` file and keeps its last content while a non-Markdown file is active. The program now runs as an iced *daemon* (to support the second window), so closing the main window exits the program through the IDE's own handler.
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
