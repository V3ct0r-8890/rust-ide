# rust-ide

A Turbo Pascal–style IDE for **Rust**, **Python** and **C#**, with a GUI (iced) and, later, a TUI front end.

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

| Job | Rust | Python | C# |
|---|---|---|---|
| Build / check | `cargo build --message-format=json` | `py_compile` / `ruff` | `dotnet build` (the project, or the single file) |
| Run | `cargo run` | `python` / `pythonw` | `dotnet run` |
| Code intelligence | `rust-analyzer` (LSP) | `pyright` / `ruff` (LSP) | `csharp-ls` (LSP) |
| Debug | `codelldb` (DAP) | `debugpy` (DAP) | not yet |

## Keys

Everything works from the keyboard. **F1 lists every shortcut.**

| Key | Action |
|---|---|
| F1 | Keyboard shortcut list |
| F10 / Alt+letter | Open a menu (Alt+F File, Alt+E Edit, Alt+S Search, Alt+R Run, Alt+C Compile, Alt+O Options, Alt+H Help); arrows and Enter, or the red letter, choose an item |
| F2 / Ctrl+S | Save (asks for a name the first time) |
| F3 / Ctrl+O | Open (each file in its own window) |
| F9 | Compile, list errors, jump to the first one |
| Alt+F9 | Syntax check: find errors quickly without building or running (Rust: `cargo check` / `rustc` metadata, Python: syntax parse) |
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
| Alt+F6 / Alt+P | Markdown preview on / off (for `.md` files), where Options > Preview in says |
| Ctrl+= / Ctrl+- / Ctrl+0 | Font larger / smaller / default (window version; remembered) |
| Alt+X | Exit (press twice to discard unsaved changes) |

**Window style** (window version): **Options > Window style** (Alt+O, then L / H / T) chooses how open files are
shown: *Legacy windows* (Turbo Pascal: a frame line with the file name and the window number, windows chosen with F6,
Alt+1-9 and the Window menu), *Horizontal tabs* (the default: a tab strip under the menu bar) or *Vertical tabs* (a list of
the open files on the left of the editor). The tab styles appear once two or more files are open. The keys are the same
in every style, and the choice is remembered.

**Markdown preview** (window version): in a `.md` window, **Alt+F6** (or Alt+P, or Window > Markdown preview) shows the
rendered document next to the editor: headings, bold and italic, lists, quotes, tables, code blocks and links. It follows
what you type and scrolls with the cursor; clicking a link opens web addresses in the browser and other `.md` / text files
in a window. Images show only their description for now.

**C#** (0.2.21): `.cs` files are highlighted; F9 / Alt+F9 run `dotnet build` on the file's project (the nearest folder above it with a `.csproj`), or on the single file when there is no project (a file-based app, needs the .NET SDK 10 or newer); errors and warnings go to the Messages panel with their `CS` codes, and F5 runs the program in the Output window. Install the SDK with Options > Dependencies (`winget install Microsoft.DotNet.SDK.10` on Windows). Live errors and completion (window version) come from `csharp-ls` (Options > Dependencies installs it with `dotnet tool install -g csharp-ls`) for files inside a `.csproj` / `.sln` folder; a loose single `.cs` file gets highlighting and F9 but no live errors. The debugger is still to come.

**Settings dialog** (window version): **Options > Settings...** (Alt+O E) has the main options in one place: where the Markdown preview opens, window style, Vim mode, check before run, font size and the dependencies check. Up/Down choose a row, Left/Right or Enter change it, a letter (P W V C F D) uses that row, Esc closes; each change is applied and remembered at once. The menu items and keys below still work.

**Where the preview opens** is chosen in **Options > Preview in** (Alt+O P cycles it; remembered):
- *side by side* (default): next to the editor;
- *a new tab*: Alt+F6 opens a `Preview name.md` tab after the file. It shows the text as it was when you selected the tab, and is read again each time you come back to it. Alt+F6 on the preview tab closes it and returns to the file; on the file it jumps to the open preview tab. The preview tab cannot be edited or saved;
- *a new window*: Alt+F6 opens a separate OS window (move it to a second screen); it follows what you type in the active `.md` file. Alt+F6 or the window's close button closes it.

**Starting without a file** (no file name on the command line): the IDE asks which language to use: Rust (.rs),
Python (.py), C# (.cs), or, for any other language, Text (.txt) or Markdown (.md). Press R / P / C / T / M or 1-5 (or click), Enter takes
the highlighted one (the one you chose last time), Esc gives a plain Rust window. **File > New** asks the same question.

**File > New project...** (Alt+F P; both versions) creates a ready-made project: pick Rust, Python or C#, a size, a project name and a folder (Tab / Up / Down move between the fields, Left/Right or the letters change a choice, Enter creates, Esc cancels). The new project folder is `folder/name`, and its main file opens, ready for F5. Sizes:

| Size | Rust | Python | C# |
|---|---|---|---|
| Simple | Cargo project, one `main.rs` | one `main.py` | console project, one `Program.cs` |
| Low | binary crate in modules (`greeting`, `stats`) with unit tests | package with `__main__.py` and modules, pytest tests | console project in a few classes (`Greeting`, `Stats`) |
| Medium | library + binary, error type, `config.txt`, integration tests, README | `pyproject.toml`, `src/` layout, `config.toml`, pytest, ruff settings, README | solution: library + console app, `config.txt`, xunit tests, README |
| Complex | Cargo workspace (util, core, cli), tests, a benchmark, CI workflow | `src/` package with argparse CLI, config, logging, tests, CI workflow | solution: util, core and cli projects, `Directory.Build.props`, xunit tests, CI workflow |

Rust projects need only Cargo; the C# ones need the .NET SDK 10 (xunit packages download on the first test run); the Python ones need `pytest` for their tests (`pip install pytest`; Medium and Complex list it in `pyproject.toml`).
The choice decides the highlighting, the `NONAME` file name and what Save As offers. `.txt` and `.md` files open like
any other file, without highlighting.

The Output window shows the program's output and has an input line for programs that read
from the keyboard. *Run > Run in external terminal* opens a separate terminal instead.

Click a line in the Messages panel to jump to that error. **Options > Check before run** (window version) makes
F9 and F5 run the syntax check first and stop on errors, so a build or run never starts with broken code. While you type, the language server's
errors are shown live: red/magenta text in the editor, the "Messages (live)" panel, and the status bar. `rust-ide a.rs b.py` opens files at startup, each in its own window.

## Terminal version

`rust-ide-tui` is the same IDE in a terminal (Windows Terminal, conhost, GNOME Terminal,
Konsole, xterm, SSH sessions): Turbo Pascal colors, block cursor, F10 / Alt+letter menus,
F1 help, several windows (F6, Alt+1-9), Ctrl+F / Ctrl+L / Ctrl+G, F9 compile with a
Messages panel (Alt+F8 / Alt+F7), and **F5 runs the program right in the terminal**,
like Turbo Pascal's user screen (Alt+F5 shows it again).

- **Replace** (Ctrl+H, or Ctrl+R because many terminals send Ctrl+H as Backspace; Search > Replace...): asks for the text and then the replacement, replaces every match (case ignored, like Find) and Ctrl+Z undoes it in one step.
- **Options > Check before run**: F9 / F5 first look for errors and stop if there are any (shared setting with the window version).
- **Options > General / Project environment / Show environment**: see "Environment variables" below (same in the window version).
- **Open and Save As start in the folder you used last** (last_dir in the settings file): the active file's folder, else the last folder of a file opened or saved.
- **Options > Line numbers** (Alt+O L in the terminal version, Alt+O N in the window version, also a Settings row): a column of line numbers left of the text; remembered.
- **Mouse**: click a menu title, a menu item, a window tab or a place in the text; the wheel moves the cursor. Start with `--no-mouse` if you would rather select text with the terminal's own mouse selection.

```
rust-ide-tui hello.rs main.py
```

Not in the terminal version yet: live errors and completion (language servers), the
debugger, Vim mode and a Settings dialog (the Options menu has only Check before run so far). Set `NO_COLOR` and it shows no colors, as terminal
programs should.

## Environment variables

Programs the IDE starts (builds, runs, the debugger, language servers) can get extra environment
variables, in two groups kept apart. Both are plain text files with one `NAME=value` per line
(`#` comments, quotes allowed; `$NAME`, `${NAME}` and `%NAME%` use existing values, so
`PATH=C:	ools;${PATH}` adds to the path). Edits apply at the next build or run, no restart.

- **General** (**Options > General environment...**, Alt+O G): for every project - proxy settings
  (`HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY`), extra `PATH` entries, tool folders. The file is
  `environment.env` next to the IDE's settings file; it starts with a commented template.
- **Project** (**Options > Project environment...**, Alt+O O): for the project of the open file
  (`.rust-ide/env` in the project folder: the Git root, else the nearest folder with a
  `Cargo.toml`, `pyproject.toml`, `requirements.txt` or `.venv`). A project value wins over a
  general one with the same name. If the project has a `.gitignore`, `.rust-ide/` is added to it so
  secrets are not committed.
- **Options > Show environment** (Alt+O X) opens a window listing both groups and where each file is.
- **Options > Python .venv setup** (Alt+O Y): creates `.venv` in the project folder if there is none, then
  installs `requirements.txt` (or the project itself with `pip install -e .` when it has a
  `pyproject.toml` / `setup.py`). F5 and the syntax check then use the `.venv` interpreter (the
  debugger and language server already did). The environment variables above apply to pip too.
- **Options > Rust toolchain...** (Alt+O U): a list of the toolchains `rustup` has installed, with a
  hint on each row; Up/Down and Enter pick one, **Default** goes back to rustup's default, and
  **Other name...** (O) lets you type one. The choice is saved as `RUSTUP_TOOLCHAIN=` in the project
  environment, so builds, runs, the debugger and rust-analyzer use it (reopen the file to restart
  the language server). A `rust-toolchain.toml` / `rust-toolchain` file in the project is honoured
  too; the IDE's own choice wins. The status bar shows `Rust <toolchain>` while a Rust file is
  open, and Show environment names where the choice comes from.

### Which Rust toolchain?

| Name | Meaning |
|---|---|
| `stable` | The latest release. The normal choice. |
| `beta` | The next release, for trying it early. |
| `nightly` | Newest features, can break; some crates need it. `nightly-2025-01-01` pins a date. |
| `1.85.0` | A pinned version, for a project that must build with exactly that compiler. |

- **Windows:** every toolchain comes in two flavours. **`-x86_64-pc-windows-gnu`** links with
  MinGW and is what this IDE is built and tested with. **`-x86_64-pc-windows-msvc`** needs Visual
  Studio's `link.exe`; without it builds fail with "linker `link.exe` not found". The picker shows
  such a toolchain in red, and a bare name typed under "Other name..." gets `-x86_64-pc-windows-gnu`
  added. Linux and macOS have only one flavour per release.
- **Install more** in a terminal (the IDE lists, it does not install):
  `rustup toolchain install nightly-x86_64-pc-windows-gnu` (Windows) or
  `rustup toolchain install nightly` (Linux). Remove with `rustup toolchain uninstall <name>`.

## Sessions

When you close the IDE (Alt+X, closing the window, or `:q` in Vim mode) it remembers the files that
were open, the cursor of each and the window that was active. Start it again **without file names**
and they come back; the language chooser only appears when there is nothing to restore. Files
you give on the command line are opened as given and no session is restored.

- The session belongs to the project: the one of the folder you start in, else the one used last
  (so a shortcut to the program still brings back your work). It is kept in the IDE's settings
  folder (`sessions\` next to `settings.txt`), never in your project.
- Files that no longer exist are skipped; unsaved new files are not remembered.
- To turn it off, put `restore_session=false` in the settings file.
## Vim mode

Turn it on in **Options > Vim mode** (remembered between sessions). The status bar shows
`NORMAL`, `-- INSERT --`, `-- VISUAL --` or `-- VISUAL LINE --` and any pending command. Supported:

- Modes: `i a I A o O s S` to insert, `Esc` back to Normal, `v` for Visual
- Motions (with counts): `h j k l w b e 0 ^ $ gg G`, `Ctrl+D`/`Ctrl+U`
- Find on the line: `f{c} t{c} F{c} T{c}` (e.g. `dt)`, `2f,`), `;` repeats, `,` reverses
- Operators with motions: `d c y > <` (e.g. `dw`, `c$`, `y2j`, `dG`), doubled for lines (`dd`, `3yy`, `>>`)
- Text objects after an operator: `iw aw iW aW`, quotes `i" a" i' a'`, backticks, `i( a( ib ab`, `i[ a[`, `i{ a{ iB aB`, `i< a<` (e.g. `ciw`, `di(`, `da"`, `yi{`)
- Editing: `x X D C Y p P J r<char> u Ctrl+R`
- Repeat: `.` repeats the last change made in Normal mode (`dw`, `ciw` + typed text, `A` + text, `x`, `p`, ...)
- Marks: `m{a-z}` sets one, `'{a-z}` jumps to its line, `` `{a-z} `` to its exact place; they work with operators (`d'a`) and do not move when text changes
- Visual: `v` (characters) or `V` (whole lines); motions, `iw`/`i(`-style objects, `o` to swap ends, then `d x y c > <`
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

Done: more Vim commands, font size, Markdown preview, window style (legacy / horizontal / vertical tabs), syntax check (Alt+F9), terminal version `rust-ide-tui`, several open files (tabs, F6, Window menu), debugger (breakpoints, F7/F8 stepping, watches, locals, call stack), editor with a Turbo-style block cursor and full keyboard control (F1 help, F10 menus), build and run (F9/F5) with the in-IDE Output window, highlighting, live errors
and completion, undo, basic Vim mode, Search menu (find, replace, go to line),
Windows and Linux packages.

Next, easiest first:
the rest of the terminal version (including a Settings dialog), C# support.

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
