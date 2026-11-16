# Status and roadmap

Last updated: 2026-10-20. Latest release: [v0.2.0](https://github.com/V3ct0r-8890/rust-ide/releases/tag/v0.2.0).

## Done

| Area | What works |
|---|---|
| Editor | Turbo Pascal-style GUI (iced): menu bar, blue editor, status line, File menu, F2/F3/Alt+X, unsaved-changes guard (0.2.55, both versions: the window's X button and Vim `:q`/`:wq` ask too, Save in the Exit dialog saves every changed window, windows cannot be switched while the dialog is open) |
| Build and run | F9 compile/check, F5 build and run for Rust (cargo or single-file rustc) and Python; Messages panel that jumps to errors |
| Output window | Programs run inside the IDE (Alt+F5 show/hide, Esc hides, Ctrl+F2 stops), with an input line for programs that read the keyboard; *Run in external terminal* as an alternative |
| Highlighting | Rust and Python, multi-line comments and strings |
| Language servers | rust-analyzer and basedpyright/pyright: live errors in the editor and a live Messages panel, completion popup (Ctrl+Space, auto after `.` and `::`) |
| Editing | Undo/redo (Ctrl+Z, Ctrl+Y); Vim mode (Options > Vim mode): motions, operators, text objects (`ciw`, `di(`, `da"`), `f`/`t` with `;` `,`, `.` repeat, marks, Visual and Visual-line mode |
| Markdown preview | Window version: **Alt+F6** / Alt+P / Window menu shows the rendered `.md` next to the editor (iced's Markdown widget: headings, emphasis, lists, quotes, tables, code blocks, links); follows typing and the cursor; links open in the browser or as files. **Window > Preview in** (Alt+W I) picks where it shows: side by side, in its own tab (refreshed when you select it), or in a separate window (follows typing) |
| Window style | Window > Legacy / Horizontal / Vertical (window version; was under Options before 0.2.32): **Legacy windows** (Turbo Pascal frame line with the name and window number), **Horizontal tabs** (default) or **Vertical tabs** (list on the left); same keys in every style; the choice is remembered |
| New files | Started without a file, the IDE (both versions) asks which language to use: Rust, Python, or Text / Markdown for anything else; File > New asks too. The last choice is remembered and preselected; the type sets highlighting, the `NONAME.xx` name and the Save As default; `.txt` and `.md` open as plain files |
| New project | **File > New project...** (Alt+F P, both versions): Rust, Python or C#, size Simple / Low / Medium / Complex, a name and a folder; writes the project and opens its main file. All 12 templates are built, run and tested by `cargo test templates:: -- --ignored` (run in CI) |
| Environment variables | **General environment... / Project environment... / Show environment** (Tools menu in both versions; 0.2.14, terminal Tools menu 0.2.53): `NAME=value` files for proxy, PATH and project variables, applied to everything the IDE starts; see the README |
| Terminal: Replace, mouse, Options | `rust-ide-tui` (0.2.12): Replace all (Ctrl+H / Ctrl+R, one undo), mouse (menus, tabs, cursor, wheel; `--no-mouse` turns it off), Options > Check before run, Options > Line numbers (0.2.13) |
| Settings dialog | Window version: **Options > Settings...** (Alt+O S) - preview place, window style, Vim mode, check before run, font size, dependencies; keyboard (Up/Down, Left/Right, Enter, letters, Esc) and mouse; changes apply and are remembered at once. Terminal version (0.2.53): Options > Settings... with check before run, line numbers, theme, two-line status and Dependencies |
| Last folder | Open / Save As dialogs start in the active file's folder, else the folder of the file opened or saved last (last_dir, 0.2.20; both versions) |
| Line numbers | **Options > Line numbers** (Alt+O N in both versions since 0.2.54; also a Settings row), remembered in line_numbers. The window version reserves a column inside the BlockCursor wrapper (src/cursor.rs) and derives the scroll position from the caret rectangle the editor reports, counting wrapped rows; the current line is highlighted. Until the editor has shown a caret, the numbers start at line 1 |
| Menu font size | 0.2.43, window version: Options > Menu font small / medium / large (`menu_font=small|medium|large`, default large = the old look). Scales the menu bar and drop-down text and their geometry (`menu_points()`, `menu_scale()`); dialogs and the editor keep their size (the editor has Ctrl+= / Ctrl+-). Not done: a Settings dialog row |
| Menus and context menu | 0.2.48, window version: `separator()` items group Edit/Tools/Options; Options > Theme cycles turbo/mono/modern, Options > Menu font cycles sizes; Edit has Cut/Copy/Paste/Select all/Remove double empty lines (`remove_empty_lines`, runs shrink to one); word completion (0.2.51, `local_items`, `complete_locally`: file words by use, then keywords, merged after language-server items); right-click in the editor (BlockCursor `on_right_click`) opens `context_items` at the mouse; modern selection colour is light blue. Terminal version 0.2.53: the same menus (Tools, separators that the arrows skip), selection (`Buffer::anchor`; Shift+keys, drag, Shift+click), Cut/Copy/Paste/Select all (`clipboard.rs`: clipboard-win on Windows, OSC 52 plus wl-copy/xclip/xsel elsewhere, bracketed paste on Unix), the right-click menu (`Mode::Context`), Remove double empty lines and word completion (`remove_empty_lines` and `local_items` moved to `src/text.rs` for both), Tools > Dependencies (`Mode::Deps`; checks and installs block the loop with a message drawn first) |
| Modern theme | Window version, 0.2.46: Options > Theme (was Modern theme) (`theme=modern`): light palette (`palette()` has a third set of colours; `editor_bg()` / `editor_fg()` for the page), dark-on-white syntax colours, system font for menus and dialogs from the next start (the editor and cursor are always monospaced) |
| Mono theme, two-line status | Window version, 0.2.33: Options > Mono theme (`theme=mono`) and Options > Two-line status (`status_lines=2`), both also Settings rows. The palette is `turbo_*()` functions that read `platform::mono()`; syntax colors switch in `highlight::to_format`. Terminal version 0.2.53: the palette in `ui.rs` is functions per theme (Turbo, Mono, Modern) reading the same `theme` setting; Options > Theme cycles; two-line status reads `status_lines` |
| Font size | Window version: **Ctrl+=** larger, **Ctrl+-** smaller, **Ctrl+0** default (also Options menu, Alt+O L / M / D); 10 to 32 points, remembered (`font_size` in the settings file). Applies to the editor (with the block cursor), the Markdown preview and the user screen; menus and panels keep their size. The terminal version uses the terminal's own zoom |
| Syntax check | Compile > Syntax check, **Alt+F9** (GUI and terminal version): finds errors without building or running - `cargo check` for projects, `rustc --emit=metadata` for single Rust files, the syntax parse for Python; errors go to the Messages panel and the cursor jumps to the first. Options > Check before run (GUI) makes F9 / F5 run it first and stop on errors |
| Debugger | Debug menu with Turbo Pascal keys: Ctrl+F8 breakpoint, F7 trace into, F8 step over, Shift+F8 step out, F4 go to cursor, Shift+F5 debug run, Ctrl+F7 watch, Ctrl+F2 reset. Rust through `gdb -i dap` (gdb 14+), Python through debugpy. Breakpoints and the execution line are colored in the editor; the debug panel shows watches, arguments and locals, and the call stack |
| Terminal version | `rust-ide-tui` (ratatui + crossterm): Turbo colors, block cursor, menus with hotkeys, F1 help, windows, find / search again / go to line, F9 compile with messages, open-file dialog. Shares the build runner, search and lexers with the GUI through the library crate. 0.2.54: the window version's menus and keys, Output window, debugger, live errors and completion, Vim mode, Markdown preview (side or tab), window styles, search bar |
| Windows | Several open files: tabs under the menu bar, F6 / Shift+F6, Alt+1-9, Window menu (Alt+0) with the window list, Ctrl+F3 close (Alt+F3 in the terminal version; the window version dropped it in 0.2.34 because other programs use it); each window has its own text, undo and diagnostics; F3, error jumps and the debugger open files in windows; `rust-ide a.rs b.py` opens several files |
| macOS | 0.2.37: `Os::MacOs` paths and hints (Library/Application Support, Homebrew dirs, Terminal.app for external runs, brew install hints, managed Python/Rust for apple-darwin), `packaging/package-macos.sh` (Rust IDE.app + rust-ide-tui, ad-hoc signed, Apple Silicon only; the Intel build was dropped on request), macOS jobs in CI and release. **Never run on a Mac yet**: only type-checked with `cargo check --target aarch64-apple-darwin`. Not notarized. Rust debugging (gdb) is not offered; lldb-dap would be the way |
| Lessons | 0.2.41: Help > Learn Rust / Python / C# (GUI and terminal version): 25 lessons per language (`lessons/<lang>/NN_name.ext`, embedded with `include_str!` in `src/lessons.rs`), listed in a dialog (arrows, PgUp/PgDn, Left/Right switch language, Enter opens). `lessons::open` writes the file into `<data folder>/rust-ide/lessons/<lang>/` once and keeps the user's edits; `RUST_IDE_LESSONS_DIR` redirects it (tests). Every lesson was compiled or run by hand; `cargo test lessons:: -- --ignored` repeats that. Not done: lesson progress marks, an "reset lesson" item, translations |
| Keyboard | Everything reachable from the keyboard: F1 shortcut list, F10 / Alt+letter menus with arrow keys and red hotkey letters, Dependencies dialog keys, Alt+F8/Alt+F7 through messages, Tab/Shift+Tab indent; Turbo-style block cursor; the editor has focus at startup |
| Search | Search menu: Find (Ctrl+F) with all matches highlighted and a match count, Replace / Replace all (Ctrl+H, undoable), Search again (Ctrl+L), Go to line (Ctrl+G) |
| Platforms | Windows 10/11 (GNU toolchain, no Visual Studio), Ubuntu 24.04+, Fedora, RHEL/Rocky/AlmaLinux 9 and 10 |
| Setup help | Tools > Dependencies checks every tool, shows per-OS install commands, installs user-level tools, links to [INSTALL.md](INSTALL.md); Linux checks GUI libraries before opening its window |
| Releases | Windows zip and Linux tar.gz with install scripts; CI on 5 systems; tagging `vX.Y.Z` publishes a release |

## Backlog

Ordered from easiest to hardest. Effort: **S** = small, **M** = medium, **L** = large.

| # | Item | Effort |
|---|---|---|
| 1 | Development environments for project sessions: variables (0.2.14), Python .venv (0.2.15) and Rust toolchain (0.2.16, picker 0.2.17) and sessions (0.2.18) are done, the rest is **next** | L |
| 2 | ~~Terminal version: the rest~~ - done in 0.2.54 | L |
| 3 | C# support: debugger (build / run / highlighting 0.2.21, language server 0.2.22) | L |

### 1. Development environments for project sessions - L (variables, Python .venv and Rust toolchain done, the rest is next)
**Phase 1 is done in 0.2.14:** both groups of variables as plain `NAME=value` files
(`environment.env` next to the settings file, and `.rust-ide/env` in the project; a project value
wins; `$NAME` / `${NAME}` / `%NAME%` expand), applied to builds, runs, the debugger, language
servers and tool checks, with Options > General environment / Project environment / Show
environment in both front ends. Format decision taken: plain `NAME=value`, not TOML. **Phase 2 (0.2.15):** Options > Python .venv setup creates the project's `.venv`, installs its
requirements, and F5 / syntax check use it. **Phase 3 (0.2.16):** Options > Rust toolchain
chooses the toolchain per project from a list of the installed ones (0.2.17; bare names get the GNU host on Windows; `RUSTUP_TOOLCHAIN` in the project file; `rust-toolchain.toml`
is honoured), shown in the status bar. **Sessions (0.2.18):** the open files, cursors and active window are saved on exit and restored when started without files (per project, in the settings folder); the target is not a separate setting yet (set
`CARGO_BUILD_TARGET` in the project file). Not done yet:
the combined dialog with "where each value comes from", and everything below.
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
  **Done in 0.2.25 (Python and Rust):** two opt-in rows in Tools > Dependencies, "Managed Python"
  (python-build-standalone 3.13, checksum from the release's `SHA256SUMS`) and "Managed Rust"
  (`rustup-init` with the stable minimal profile + rust-analyzer, GNU host on Windows, checksum from
  its `.sha256`), installed with `curl` and `tar` under `%LOCALAPPDATA%\rust-ide\tools` (Linux
  `~/.local/share/rust-ide/tools`; `RUST_IDE_TOOLS` overrides). Managed tools are found before PATH
  and get `CARGO_HOME` / `RUSTUP_HOME` automatically. Not yet: updating them, removing them, the
  MinGW gcc for Windows, .NET and the other helper tools. Managed Rust has not been installed end to
  end (about 300 MB); managed Python has (`cargo test python_installs -- --ignored`).
- **Sessions:** the open windows, cursor positions, window style and the project's environment are
  saved when the IDE closes and restored next time (per project folder).
- **Both front ends** read the same environment file; the window version gets the dialog first.
- To confirm: the format and place of the project file (`.rust-ide/env.toml` or `.env`), whether
  general variables may also be set per user vs per machine, and whether the managed download
  should be opt-in.

### 2. Terminal version: the rest - L
Suggested sub-steps, easiest first:
1. ~~Replace, mouse support and an Options menu~~ - done in 0.2.12 (Ctrl+H / Ctrl+R replace all, click menus / tabs / text, Options > Check before run).
   ~~Menus, selection and clipboard, right-click menu, themes, two-line status, word completion,
   Settings and Dependencies dialogs~~ - done in 0.2.53.
   Cannot be ported: font size, menu font and the system font (the terminal sets the font), a
   separate preview window, images. Limited by terminals: some do not pass Ctrl+V, Ctrl+Space,
   Alt+F9 or Shift+F5 to programs; OSC 52 copying does not work in the classic Windows console
   (the Windows clipboard API is used there instead).
2. ~~Vim mode, live errors and completion, the debugger, the Markdown preview, the Output
   window, Run in external terminal, window styles, the search bar (match case, previous,
   replace one), Help > Install guide / Project page, the startup dependency check and the
   full Settings dialog~~ - done in 0.2.54. The main loop (`main.rs`) polls keys every 40 ms and
   drains an mpsc channel fed by threads that run the library's streams (`lsp::connect`,
   `output::run`, `dap::run`) with `block_on_stream`; `live.rs` turns their events into state.
   Vim runs the shared engine on an iced `Content` copy of the buffer (`vimkeys.rs`); the preview
   is `markdown.rs` (pulldown-cmark to styled lines); `dap::launch_for` is shared with the window
   version. Builds and dependency checks still block the loop (with a message drawn first).

### 3. C# support (open-source tools only) - L
**Phase 1 is done in 0.2.21:** C# in the language list (`.cs`), a C# lexer (keywords, `$"..."`, `@"..."`,
raw `"""` strings, `#region`), F9 / Alt+F9 through `dotnet build` (the nearest `.csproj`, else the single
file as a file-based app on .NET 10+), MSBuild messages (`file(line,col): error CS...`) in the Messages
panel, F5 through `dotnet run`, and ".NET SDK 10" in Tools > Dependencies. Both versions.
- **Language server - done in 0.2.22:** `csharp-ls` (MIT, `dotnet tool install -g csharp-ls`, listed in
  Tools > Dependencies) for live errors and completion; its root is the nearest `.sln` / `.csproj` folder,
  so loose single files get none. Live test: `dotnet tool install csharp-ls --tool-path .dotnet-tools` then
  `cargo test csharp_ls_live -- --ignored`.
- **Debugger - done in 0.2.24:** `netcoredbg` (Samsung, MIT), which speaks the Debug Adapter Protocol like gdb and
  debugpy (Microsoft's `vsdbg` is not open source and is not used). Window version: F7/F8, breakpoints,
  locals, Go to cursor on a `.csproj` project's last Debug build (F9 first); it is found in `.dotnet-tools`
  next to the project, on PATH, or via `NETCOREDBG`. Single files (file-based apps) cannot be debugged yet,
  (the terminal version uses the same launch code since 0.2.54). Live test: put netcoredbg in `.dotnet-tools/netcoredbg`
  then `cargo test netcoredbg -- --ignored`. Not yet tried on Linux.
- **Setup:** csharp-ls and netcoredbg in Tools > Dependencies, the install scripts and docs/INSTALL.md;
  Ubuntu `sudo apt install dotnet-sdk-10.0` (or Microsoft's package feed) and Fedora/RHEL
  `sudo dnf install dotnet-sdk-10.0` are listed but not yet tried on those systems; netcoredbg from its
  GitHub releases.
- **Templates - done in 0.2.23:** C# in File > New project (both versions): Simple (console project), Low (a few classes),
  Medium (`.slnx` solution: library + console app + xunit tests), Complex (util / core / cli + tests +
  `Directory.Build.props` + CI workflow). Tested by `cargo test templates:: -- --ignored` (skipped without .NET 10).

## Known limitations


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
- **Not yet tested with a real keyboard:** Alt+F5 and typing into the Output window, and the Install button in Tools > Dependencies. Automated tests send synthetic keys, which Windows handles differently.
- **No LICENSE file yet.** Until one is added, others may view the code but have no clear right to reuse it.

## For contributors

- Build: `cargo run`; tests: `cargo test`; live language-server tests: `cargo test -- --ignored`.
- Packages: `packaging/package-windows.ps1`, `sh packaging/package-linux.sh`.
- Long-running work (builds, tool checks) must run on a worker thread (`platform::blocking`). Blocking the GUI's async executor makes Wayland compositors drop the window.
- iced editor columns are **byte** offsets in the line, not characters. Convert when talking to compilers and language servers.
