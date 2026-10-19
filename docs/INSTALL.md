# Installing rust-ide and its tools

rust-ide drives tools you install separately. When something is missing,
it opens **Tools > Dependencies** at startup, where it lists each tool,
shows the install command, and can install some of them for you.
This page lists the same steps for each supported system.

| Tool | Needed for | |
|---|---|---|
| Rust toolchain (`cargo`, `rustc`) | building and running Rust | required |
| C compiler / linker (`cc`) | linking Rust programs (Linux) | required on Linux |
| Python 3 | checking and running Python | required for Python |
| rust-analyzer | live Rust errors and completion | recommended |
| gdb 14 or newer | debugging Rust (F7/F8, breakpoints) | recommended |
| debugpy | debugging Python | recommended |
| netcoredbg | debugging C# (projects) | optional |
| basedpyright (or pyright) | live Python errors and completion | recommended |
| xdg-desktop-portal | Open / Save dialogs (Linux) | recommended on Linux |
| a terminal emulator | *Run > Run in external terminal* (Linux) | optional |

Programs normally run inside the IDE's **Output** window (F5, then
Alt+F5 to show or hide it), so a terminal emulator is not needed.

**Quickest way:** download the package for your system from the
[Releases page](https://github.com/V3ct0r-8890/rust-ide/releases) and run the script inside it:
`install-deps.ps1` on Windows, `./install-deps.sh` on Linux. The sections below
list what those scripts do, step by step.

- [Windows 10/11](#windows-1011)
- [Ubuntu 24.04 LTS and newer](#ubuntu-2404-lts-and-newer)
- [Fedora, RHEL 9/10, Rocky Linux, AlmaLinux](#fedora-rhel-910-rocky-linux-almalinux)
- [Building rust-ide from source](#building-rust-ide-from-source)

---

## Windows 10/11

rust-ide uses the open-source GNU toolchain, so Visual Studio is **not** needed.

```powershell
# Rust, using the GNU toolchain
winget install Rustlang.Rustup
rustup default stable-x86_64-pc-windows-gnu

# MinGW-w64 gcc + binutils (dlltool), needed by some crates
winget install BrechtSanders.WinLibs.POSIX.MSVCRT

# Python
winget install Python.Python.3.13

# Language servers
rustup component add rust-analyzer
python -m pip install --user basedpyright debugpy
```

gdb comes with the MinGW-w64 (WinLibs) package above.

Don't want to install Python or Rust yourself? Tools > Dependencies has "Managed Python" and
"Managed Rust" rows that download the IDE's own copies (needs `curl` and `tar`, both built into
Windows 10+); the IDE uses them before the ones on PATH.

netcoredbg (C# debugger, optional): download `netcoredbg-win64.zip` from
https://github.com/Samsung/netcoredbg/releases, unzip it and put the folder on PATH.

Open a new terminal afterwards so the updated `PATH` is picked up.

## Ubuntu 24.04 LTS and newer

```bash
# C compiler/linker, Python, file dialogs, pipx
sudo apt update
sudo apt install build-essential python3 pipx xdg-desktop-portal xdg-desktop-portal-gtk gdb python3-debugpy

# Rust (the apt rustc is too old for current crates; use rustup)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"

# Language servers
rustup component add rust-analyzer
pipx install basedpyright
pipx ensurepath
```

Ubuntu 24.04 blocks `pip install --user` for the system Python (PEP 668),
which is why basedpyright is installed with `pipx`. rust-ide also finds
it inside a project's `.venv`.

## Fedora, RHEL 9/10, Rocky Linux, AlmaLinux

```bash
# C compiler/linker, Python, file dialogs
sudo dnf install gcc python3 python3-pip xdg-desktop-portal xdg-desktop-portal-gtk gdb

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"

# Language servers
rustup component add rust-analyzer
python3 -m pip install --user basedpyright debugpy
```

The debugger needs gdb 14 or newer. RHEL 9 ships gdb 10; install a newer one with
`sudo dnf install gcc-toolset-14-gdb` (rust-ide finds it in `/opt/rh/`).

On KDE desktops, `xdg-desktop-portal-kde` can replace `xdg-desktop-portal-gtk`.

### Minimal or server installs

Desktop installs already have the graphics libraries rust-ide needs.
On a minimal system, add:

```bash
# Ubuntu
sudo apt install libxkbcommon0 libwayland-client0 libx11-6 libxcursor1 libxi6 libxrandr2 mesa-vulkan-drivers

# Fedora / RHEL
sudo dnf install libxkbcommon wayland libX11 libXcursor libXi libXrandr mesa-vulkan-drivers
```

---

## Building rust-ide from source

With the Rust toolchain and C compiler from above:

```bash
git clone https://github.com/V3ct0r-8890/rust-ide.git
cd rust-ide
cargo build --release
./target/release/rust-ide            # Windows: target\release\rust-ide.exe
```

`rust-ide path/to/file.rs` opens a file at startup.

## Checking your setup

Open **Tools > Dependencies** at any time. Each tool shows `[ OK ]` with
its version, or `[MISS]` with:

- **Install**: rust-ide runs the command for you (only for commands that
  need no administrator rights, such as `rustup component add` or `pipx install`)
- **Copy**: copies the command so you can run it in a terminal (for
  `sudo apt` / `sudo dnf` commands)

Clear *Check at startup* to stop the dialog from opening automatically.
