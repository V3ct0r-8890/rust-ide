//! Operating-system differences: distro detection, program lookup,
//! opening URLs and launching an external terminal.

use std::path::{Path, PathBuf};
use std::process::Command;

pub const GITHUB_URL: &str = "https://github.com/V3ct0r-8890/rust-ide";
pub const INSTALL_GUIDE_URL: &str =
    "https://github.com/V3ct0r-8890/rust-ide/blob/main/docs/INSTALL.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    /// Ubuntu, Debian and derivatives (apt).
    Debian,
    /// Fedora, RHEL, Rocky, AlmaLinux, CentOS Stream (dnf).
    RedHat,
    OtherLinux,
    MacOs,
}

impl Os {
    pub fn detect() -> Self {
        if cfg!(windows) {
            return Os::Windows;
        }
        if cfg!(target_os = "macos") {
            return Os::MacOs;
        }
        let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        Self::from_os_release(&release)
    }

    fn from_os_release(release: &str) -> Self {
        let field = |key: &str| {
            release
                .lines()
                .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
                .map(|v| v.trim_matches('"').to_lowercase())
                .unwrap_or_default()
        };
        let ids = format!("{} {}", field("ID"), field("ID_LIKE"));
        let has = |name: &str| ids.split_whitespace().any(|id| id == name);
        if has("fedora") || has("rhel") || has("centos") {
            Os::RedHat
        } else if has("debian") || has("ubuntu") {
            Os::Debian
        } else {
            Os::OtherLinux
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Os::Windows => "Windows",
            Os::Debian => "Ubuntu / Debian",
            Os::RedHat => "Fedora / RHEL",
            Os::OtherLinux => "Linux",
            Os::MacOs => "macOS",
        }
    }
}

/// A shared library the GUI loads at runtime, with its package names.
struct GuiLib {
    file: &'static str,
    apt: &'static str,
    dnf: &'static str,
}

const WAYLAND_LIBS: &[GuiLib] = &[
    GuiLib { file: "libwayland-client.so.0", apt: "libwayland-client0", dnf: "libwayland-client" },
    GuiLib { file: "libxkbcommon.so.0", apt: "libxkbcommon0", dnf: "libxkbcommon" },
];
const X11_LIBS: &[GuiLib] = &[
    GuiLib { file: "libX11.so.6", apt: "libx11-6", dnf: "libX11" },
    GuiLib { file: "libxcb.so.1", apt: "libxcb1", dnf: "libxcb" },
    GuiLib { file: "libXcursor.so.1", apt: "libxcursor1", dnf: "libXcursor" },
    GuiLib { file: "libXi.so.6", apt: "libxi6", dnf: "libXi" },
    GuiLib { file: "libXrandr.so.2", apt: "libxrandr2", dnf: "libXrandr" },
    GuiLib { file: "libxkbcommon.so.0", apt: "libxkbcommon0", dnf: "libxkbcommon" },
    GuiLib { file: "libxkbcommon-x11.so.0", apt: "libxkbcommon-x11-0", dnf: "libxkbcommon-x11" },
];

/// Before any window exists, checks that a display is available and that the
/// libraries the GUI loads at runtime are installed. Returns a message for
/// the terminal when something is missing.
pub fn gui_preflight() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Ok(());
    }
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
    let x11 = std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty());
    if !wayland && !x11 {
        return Err("No graphical display found (neither WAYLAND_DISPLAY nor DISPLAY is set).\n\
                    Start rust-ide from a desktop session."
            .into());
    }
    // winit prefers Wayland when it is available.
    let libs = if wayland { WAYLAND_LIBS } else { X11_LIBS };
    let arch = std::env::consts::ARCH;
    let dirs = [
        format!("/usr/lib/{arch}-linux-gnu"),
        format!("/lib/{arch}-linux-gnu"),
        "/usr/lib64".to_string(),
        "/lib64".to_string(),
        "/usr/lib".to_string(),
        "/lib".to_string(),
    ];
    let missing: Vec<&GuiLib> = libs
        .iter()
        .filter(|lib| !dirs.iter().any(|dir| Path::new(dir).join(lib.file).exists()))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }

    let files: Vec<&str> = missing.iter().map(|l| l.file).collect();
    let command = match Os::detect() {
        Os::RedHat => {
            format!("sudo dnf install {}", missing.iter().map(|l| l.dnf).collect::<Vec<_>>().join(" "))
        }
        _ => format!("sudo apt install {}", missing.iter().map(|l| l.apt).collect::<Vec<_>>().join(" ")),
    };
    Err(format!(
        "rust-ide cannot open its window: missing {}.\n\nInstall them with:\n    {command}\n\n\
         Install guide: {INSTALL_GUIDE_URL}",
        files.join(", ")
    ))
}

/// Runs slow, blocking work (spawning tools, builds) on its own thread so it
/// never stalls the async executor that drives the GUI.
pub async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    receiver.await.expect("worker thread panicked")
}

/// The Python interpreter command (`python3` on Linux, where `python` may not exist).
pub fn python() -> &'static str {
    if cfg!(windows) { "python" } else { "python3" }
}

/// Builds a command that never flashes a console window on Windows.
pub fn hidden_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command.envs(crate::env::general());
    command
}

/// Directories searched for tools: PATH plus the usual per-user install
/// locations that are often missing from a GUI app's PATH.
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    if let Some(home) = home_dir() {
        dirs.push(home.join(".cargo").join("bin"));
        dirs.push(home.join(".local").join("bin")); // pip --user, pipx
    }
    if cfg!(windows) {
        // The saved PATH is current even when this process inherited an
        // older one (e.g. started from a shell opened before an install).
        dirs.extend(windows_saved_path().iter().cloned());
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            dirs.push(local.join("Microsoft").join("WinGet").join("Links"));
            // python.org installer default: %LOCALAPPDATA%\Programs\Python\PythonXY
            if let Ok(entries) = std::fs::read_dir(local.join("Programs").join("Python")) {
                for entry in entries.flatten() {
                    dirs.push(entry.path());
                    dirs.push(entry.path().join("Scripts"));
                }
            }
        }
    }
    dirs
}

/// User and system PATH as stored in the registry, read once.
fn windows_saved_path() -> &'static [PathBuf] {
    static SAVED: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();
    SAVED.get_or_init(|| {
        let keys = [
            r"HKCU\Environment",
            r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        ];
        keys.iter()
            .filter_map(|key| {
                let output = hidden_command("reg").args(["query", key, "/v", "Path"]).output().ok()?;
                let text = String::from_utf8_lossy(&output.stdout).into_owned();
                // "    Path    REG_EXPAND_SZ    C:\a;%USERPROFILE%\b"
                let line = text.lines().find(|l| l.trim_start().starts_with("Path"))?;
                let value = line.split("REG_").nth(1)?.split_once("    ")?.1.trim().to_string();
                Some(value)
            })
            .flat_map(|value| {
                value.split(';').filter(|p| !p.is_empty()).map(expand_env).collect::<Vec<_>>()
            })
            .map(PathBuf::from)
            .collect()
    })
}

/// Expands `%NAME%` references using the current environment.
fn expand_env(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => out.push_str(&format!("%{name}%")),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Finds an executable by name in `dirs`.
pub fn find_in(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    let extensions: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat"] } else { &[""] };
    dirs.iter().find_map(|dir| {
        extensions
            .iter()
            .map(|ext| dir.join(format!("{name}{ext}")))
            .find(|candidate| candidate.is_file())
    })
}

pub fn find_program(name: &str) -> Option<PathBuf> {
    find_in(&search_dirs(), name)
}

/// Opens a URL in the default browser.
pub fn open_url(url: &str) -> std::io::Result<()> {
    let mut command = if cfg!(windows) {
        let mut c = hidden_command("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    command.spawn().map(|_| ())
}

/// Terminal emulators tried on Linux, with the flag that precedes the command.
const LINUX_TERMINALS: &[(&str, &[&str])] = &[
    ("x-terminal-emulator", &["-e"]),
    ("ptyxis", &["--"]),
    ("kgx", &["--"]),
    ("gnome-terminal", &["--"]),
    ("konsole", &["-e"]),
    ("xfce4-terminal", &["-x"]),
    ("mate-terminal", &["-x"]),
    ("tilix", &["-e"]),
    ("alacritty", &["-e"]),
    ("kitty", &[]),
    ("wezterm", &["start", "--"]),
    ("xterm", &["-e"]),
];

pub fn linux_terminal() -> Option<(PathBuf, &'static [&'static str])> {
    let dirs = search_dirs();
    LINUX_TERMINALS
        .iter()
        .find_map(|(name, flags)| find_in(&dirs, name).map(|path| (path, *flags)))
}

/// A command that runs `program args` in a new terminal window that stays
/// open until a key is pressed.
pub fn external_terminal(program: &Path, args: &[String]) -> Result<Command, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        let quote = |s: &str| format!("\"{s}\"");
        let mut line = quote(&program.to_string_lossy());
        for arg in args {
            line.push(' ');
            line.push_str(&quote(arg));
        }
        let mut command = Command::new("cmd.exe");
        command
            .raw_arg(format!(
                "/v:on /c \"{line} & echo. & echo [Process exited with code !errorlevel!] & pause\""
            ))
            .creation_flags(CREATE_NEW_CONSOLE);
        Ok(command)
    }
    #[cfg(not(windows))]
    {
        let (terminal, flags) = linux_terminal()
            .ok_or("No terminal emulator found (install gnome-terminal, konsole or xterm)")?;
        let mut script = shell_quote(&program.to_string_lossy());
        for arg in args {
            script.push(' ');
            script.push_str(&shell_quote(arg));
        }
        script.push_str(
            "; echo; echo \"[Process exited with code $?]\"; printf 'Press Enter to close...'; read _",
        );
        let mut command = Command::new(terminal);
        command.args(flags.iter()).args(["sh", "-c", &script]);
        Ok(command)
    }
}

#[cfg(not(windows))]
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Per-user settings file (`key=value` lines).
pub fn config_path() -> Option<PathBuf> {
    // A fixed settings file, for tests and portable setups.
    if let Some(file) = std::env::var_os("RUST_IDE_CONFIG_FILE") {
        return Some(PathBuf::from(file));
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".config")))
    }?;
    Some(base.join("rust-ide").join("settings.txt"))
}

pub fn read_setting(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(config_path()?).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
        .map(str::to_string)
}

pub fn write_setting(key: &str, value: &str) -> std::io::Result<()> {
    let path = config_path().ok_or_else(|| std::io::Error::other("no config directory"))?;
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| !l.starts_with(&format!("{key}=")))
        .map(str::to_string)
        .collect();
    lines.push(format!("{key}={value}"));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, lines.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_env_references() {
        // SAFETY: single-threaded use of a variable only this test reads.
        unsafe { std::env::set_var("RUST_IDE_TEST_DIR", "C:\\tools") };
        assert_eq!(expand_env("%RUST_IDE_TEST_DIR%\\bin"), "C:\\tools\\bin");
        assert_eq!(expand_env("%NOPE_NOT_SET%\\x"), "%NOPE_NOT_SET%\\x");
        assert_eq!(expand_env("plain;50%"), "plain;50%");
    }

    #[test]
    fn detects_distros() {
        let ubuntu = "NAME=\"Ubuntu\"\nID=ubuntu\nID_LIKE=debian\nVERSION_ID=\"24.04\"";
        assert_eq!(Os::from_os_release(ubuntu), Os::Debian);
        let rhel = "NAME=\"Red Hat Enterprise Linux\"\nID=\"rhel\"\nID_LIKE=\"fedora\"";
        assert_eq!(Os::from_os_release(rhel), Os::RedHat);
        let alma = "ID=\"almalinux\"\nID_LIKE=\"rhel centos fedora\"";
        assert_eq!(Os::from_os_release(alma), Os::RedHat);
        assert_eq!(Os::from_os_release("ID=fedora"), Os::RedHat);
        assert_eq!(Os::from_os_release("ID=arch"), Os::OtherLinux);
    }
}
