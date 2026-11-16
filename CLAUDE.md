# CLAUDE.md

Notes for AI coding sessions on this repository. Project status and backlog: `docs/ROADMAP.md`.
User-facing docs: `README.md`, `docs/INSTALL.md`.

## What this is

A Turbo Pascal-style IDE for Rust and Python, with two front ends over one library crate:
- Library (`src/lib.rs`): `runner.rs` (build/check/run), `output.rs` (in-IDE program I/O),
  `lsp.rs` (LSP client), `dap.rs` (debugger), `highlight.rs`, `search.rs`, `vim.rs`,
  `deps.rs` (dependency checker), `platform.rs` (OS differences, settings, tool lookup).
- `rust-ide` (GUI, iced 0.14): `src/main.rs` (UI, messages) and `src/cursor.rs` (block cursor).
- `rust-ide-tui` (terminal, ratatui + crossterm): `src/bin/rust-ide-tui/` (`buffer.rs` text
  buffer, `app.rs` keys and state, `ui.rs` drawing, `main.rs` terminal loop and user screen).
  Its UI is tested with ratatui's `TestBackend`. `RUST_IDE_TUI_KEYLOG=<file>` logs key events.
  The agent shell sets `NO_COLOR=1`, which turns colors off; clear it to see colors. To see it
  in a classic console window, start it through `conhost.exe` (Windows Terminal ignores
  posted keys).

## Build and test

- `cargo build`, `cargo test`. Live language-server tests: `cargo test -- --ignored`
  (needs rust-analyzer, and basedpyright in the repo's `.venv`).
- **Windows uses the GNU toolchain** (`stable-x86_64-pc-windows-gnu`) plus WinLibs MinGW for
  `dlltool`. The owner does not want Visual Studio / MSVC. Do not switch toolchains.
- Shells started by the agent may have a stale PATH. Prefix PowerShell commands with:
  `$env:Path = "$env:USERPROFILE\.cargo\bin;" + [Environment]::GetEnvironmentVariable("Path","User") + ";" + [Environment]::GetEnvironmentVariable("Path","Machine")`
- **Linux testing:** WSL `Ubuntu-24.04` is available with Rust, gcc, Python and WSLg.
  Run scripts through a file (inline `wsl.exe -- bash -c "..."` from PowerShell mangles `|` and `$`):
  `wsl.exe -d Ubuntu-24.04 -- bash -l <script>` with `. "$HOME/.cargo/env"` and
  `CARGO_TARGET_DIR=$HOME/rust-ide-target` inside, working in `/mnt/g/Development.new/Rust-IDE`.
- The Claude desktop app virtualizes `%APPDATA%` writes, so `pip install --user` from an agent
  shell is invisible to the user's own programs. Install Python tools into the repo `.venv`
  for testing (it is git-ignored), and tell the user the command to run themselves.

## GUI testing on Windows

- Windows blocks focus stealing, so `SendKeys` is unreliable and could type into the user's
  other windows. Instead, post `WM_KEYDOWN`/`WM_KEYUP` (with scan codes from `MapVirtualKey`)
  to the IDE's window handle, and capture it with `PrintWindow`.
- Synthetic keys have limits: repeated identical keys are dropped, and Alt/Ctrl state is not
  seen (Alt+F5 arrives as F5). Plain keys (F1-F12, arrows, letters) work. Synthetic mouse clicks
  are unreliable. Prefer headless tests that call `Ide::update` with messages (see the `tests`,
  `keyboard_tests` and `debug_tests` modules in `main.rs`).
- Ask the user before any test that opens windows if they might be using the machine.

## Rules learned the hard way

- **Never block the async executor.** Builds, tool checks and installs run through
  `platform::blocking(...)`. Blocking it stalled the event loop and Wayland dropped the window.
- **iced editor columns are byte offsets**, not characters. Convert when talking to compilers
  (character columns) and LSP (UTF-16, handled in `lsp.rs`). Lines may contain Thai text.
- **Positioning the cursor:** `Content::move_to` with `selection: None` keeps an old selection,
  and a Move with any selection only collapses it. Use `vim::place(content, position)`.
- `Ide::default()` (what tests use) never writes settings: saving goes through
  `Ide::save_setting`, which does nothing unless `persist_settings` is on (only `Ide::new` turns
  it on). A test that checks saving must turn it on and call `isolate_settings()`.
- Settings live in one `key=value` file (`platform::config_path`). Tests that save settings must
  point `RUST_IDE_CONFIG_FILE` at a temp file and take turns (see `isolate_settings()` in the
  `language_tests` modules); never let a test write the real file.
- **Changing the layout makes the editor lose keyboard focus** (iced rebuilds widget state by
  position): handlers that add or remove panes (preview, tabs, window style) return
  `iced::widget::operation::focus(EDITOR)`.
- Status bar text must be clipped, never allowed to push the right-hand fields off screen, so
  put the important words first.
- **F9 compiles, Alt+F9 only checks** (`runner::check`: no program is produced; `main.rs`
  `run_build` combines check and build for Options > Check before run). A run target never
  comes from a check.
- **Debugger:** `src/dap.rs` drives `gdb -i dap` (Rust, needs gdb 14+) and debugpy. gdb's built-in
  Python breaks when `MSYSTEM` (Git Bash/MSYS2), `PYTHONHOME` or `PYTHONPATH` are set, so they
  are removed for gdb. Live tests: `cargo test dap:: -- --ignored --test-threads=1`
  (parallel gdb sessions disturb each other). `DAP_TRACE=1` prints adapter commands and stderr.
- A `.rs` file belongs to a Cargo project only if it is in the package's `src/`, `examples/`,
  `tests/` or `benches/` (`runner::find_cargo_manifest`); loose files build alone with rustc.

## Releases

- CI (`.github/workflows/ci.yml`) runs on Ubuntu 24.04, Fedora, AlmaLinux 9/10 and Windows.
- Release (`.github/workflows/release.yml`): pushing a tag `vX.Y.Z` builds the Linux tar.gz on
  AlmaLinux 9 (glibc 2.34, so it runs on RHEL 9+, Fedora and Ubuntu 24.04+), checks it on
  Ubuntu/Fedora/AlmaLinux 10, builds the Windows zip with the GNU toolchain, and publishes a
  GitHub Release. Running it by hand only builds artifacts.
- Bump `version` in `Cargo.toml` before tagging. Publishing is public: confirm with the owner first.
- Local packages: `packaging/package-windows.ps1`, `sh packaging/package-linux.sh` (output in `dist/`).

## Conventions

- Repo: https://github.com/V3ct0r-8890/rust-ide (public). Work on `main`; commit and push when
  a feature is done and tested.
- Keep `README.md` (keys, features), `docs/INSTALL.md` and `docs/ROADMAP.md` current with changes.
- The owner tests builds by hand, and files they save (like `test.py`) can land in the project
  folder. Look at `git status` before committing and stage files by name; never commit them.
- **Handing work over for the user to verify (standing rule from the owner):** build the
  executables first and leave them in the project folder with the release number in the file
  names: `powershell -File packaging\package-windows.ps1` writes `dist\rust-ide-<version>.exe`,
  `dist\rust-ide-tui-<version>.exe` and the zip. Releases are plain `x.y.z`. Bump the patch number
  in `Cargo.toml` (and let `Cargo.lock` follow) for every build handed over, so the owner can tell
  builds apart; the owner picks bigger bumps and releases. State the exact path and version, and
  what to try by hand, in the hand-over message. Only Windows executables are built locally;
  Linux packages come from tagging a release (CI), so say that and offer it.
- Shell scripts keep LF endings (`.gitattributes`) and the executable bit
  (`git update-index --chmod=+x`).
