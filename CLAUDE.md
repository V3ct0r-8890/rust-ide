# CLAUDE.md

Notes for AI coding sessions on this repository. Project status and backlog: `docs/ROADMAP.md`.
User-facing docs: `README.md`, `docs/INSTALL.md`.

## What this is

A Turbo Pascal-style IDE for Rust and Python. GUI in iced 0.14 (TUI front end planned).
Modules in `src/`: `main.rs` (UI, messages), `runner.rs` (build/check/run), `output.rs`
(in-IDE program I/O), `lsp.rs` (LSP client), `highlight.rs`, `vim.rs`, `deps.rs`
(dependency checker), `platform.rs` (OS differences, settings, tool lookup).

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
- Synthetic keys have limits: repeated identical keys are dropped, and Alt state is not seen
  (Alt+F5 arrives as F5). Prefer unit tests for logic, and synthetic keys for wiring only.
- Ask the user before any test that opens windows if they might be using the machine.

## Rules learned the hard way

- **Never block the async executor.** Builds, tool checks and installs run through
  `platform::blocking(...)`. Blocking it stalled the event loop and Wayland dropped the window.
- **iced editor columns are byte offsets**, not characters. Convert when talking to compilers
  (character columns) and LSP (UTF-16, handled in `lsp.rs`). Lines may contain Thai text.
- **Positioning the cursor:** `Content::move_to` with `selection: None` keeps an old selection,
  and a Move with any selection only collapses it. Use `vim::place(content, position)`.
- Status bar text must be clipped, never allowed to push the right-hand fields off screen.

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
- Shell scripts keep LF endings (`.gitattributes`) and the executable bit
  (`git update-index --chmod=+x`).
