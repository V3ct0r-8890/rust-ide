//! Checks for the external tools the IDE drives and knows how to install
//! them on Windows, Ubuntu/Debian and Fedora/RHEL.

use std::path::Path;

use crate::lsp;
use crate::platform::{self, Os};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    Required,
    Recommended,
    Optional,
}

#[derive(Debug, Clone)]
pub struct Dep {
    pub name: &'static str,
    pub purpose: &'static str,
    pub need: Need,
    /// Version or location when installed.
    pub found: Option<String>,
    /// Command for the user to run (may need sudo/admin).
    pub install: String,
    /// A command the IDE can run itself (no admin rights needed).
    pub auto: Option<Vec<String>>,
}

impl Dep {
    pub fn missing(&self) -> bool {
        self.found.is_none()
    }
}

/// Runs `program --version` and returns its first output line.
fn version(program: &Path) -> Option<String> {
    let output = platform::hidden_command(program).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(if output.stdout.is_empty() { &output.stderr } else { &output.stdout })
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    Some(if text.is_empty() { program.display().to_string() } else { text })
}

fn program_version(name: &str) -> Option<String> {
    version(&platform::find_program(name)?)
}

fn words(command: &str) -> Vec<String> {
    command.split_whitespace().map(str::to_string).collect()
}

pub fn check(os: Os) -> Vec<Dep> {
    let python = platform::python();
    let mut deps = Vec::new();

    deps.push(Dep {
        name: "Rust toolchain",
        purpose: "compile and run Rust (cargo, rustc)",
        need: Need::Required,
        found: program_version("cargo"),
        install: match os {
            Os::Windows => "winget install Rustlang.Rustup   then   rustup default stable-x86_64-pc-windows-gnu".into(),
            _ => "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh".into(),
        },
        auto: None,
    });

    match os {
        Os::Windows => deps.push(Dep {
            name: "MinGW-w64 (gcc, dlltool)",
            purpose: "linker tools for Rust crates that need them",
            need: Need::Recommended,
            found: program_version("dlltool"),
            install: "winget install BrechtSanders.WinLibs.POSIX.MSVCRT".into(),
            auto: Some(words(
                "winget install --id BrechtSanders.WinLibs.POSIX.MSVCRT -e --accept-source-agreements --accept-package-agreements",
            )),
        }),
        _ => deps.push(Dep {
            name: "C compiler / linker",
            purpose: "Rust needs `cc` to link programs",
            need: Need::Required,
            found: program_version("cc"),
            install: match os {
                Os::Debian => "sudo apt install build-essential".into(),
                Os::RedHat => "sudo dnf install gcc".into(),
                _ => "install gcc with your package manager".into(),
            },
            auto: None,
        }),
    }

    deps.push(Dep {
        name: "Python 3",
        purpose: "check and run Python programs",
        need: Need::Required,
        found: program_version(python),
        install: match os {
            Os::Windows => "winget install Python.Python.3.13".into(),
            Os::Debian => "sudo apt install python3".into(),
            Os::RedHat => "sudo dnf install python3".into(),
            _ => "install python3 with your package manager".into(),
        },
        auto: (os == Os::Windows).then(|| {
            words("winget install --id Python.Python.3.13 -e --accept-source-agreements --accept-package-agreements")
        }),
    });

    deps.push(Dep {
        name: ".NET SDK 10",
        purpose: "build and run C# (dotnet; version 10 or newer also runs single .cs files)",
        need: Need::Optional,
        found: program_version("dotnet"),
        install: match os {
            Os::Windows => "winget install Microsoft.DotNet.SDK.10".into(),
            Os::Debian => "sudo apt install dotnet-sdk-10.0   (or add Microsoft's package feed first)".into(),
            Os::RedHat => "sudo dnf install dotnet-sdk-10.0".into(),
            _ => "install the .NET SDK with your package manager".into(),
        },
        auto: (os == Os::Windows)
            .then(|| words("winget install --id Microsoft.DotNet.SDK.10 -e --accept-source-agreements --accept-package-agreements")),
    });

    let has_rustup = platform::find_program("rustup").is_some();
    deps.push(Dep {
        name: "rust-analyzer",
        purpose: "live Rust errors and completion",
        need: Need::Recommended,
        // The rustup proxy exists even without the component, so actually run it.
        found: program_version("rust-analyzer"),
        install: "rustup component add rust-analyzer".into(),
        auto: has_rustup.then(|| words("rustup component add rust-analyzer")),
    });

    let here = std::env::current_dir().unwrap_or_default();
    let pyright = lsp::find_program(&here, lsp::PYTHON_SERVERS);
    let has_pipx = platform::find_program("pipx").is_some();
    let (install, auto) = match os {
        // Ubuntu 24.04 blocks `pip install --user` (PEP 668), so use pipx.
        Os::Debian => (
            "sudo apt install pipx   then   pipx install basedpyright".to_string(),
            has_pipx.then(|| words("pipx install basedpyright")),
        ),
        _ => {
            let command = format!("{python} -m pip install --user basedpyright");
            (command.clone(), Some(words(&command)))
        }
    };
    deps.push(Dep {
        name: "basedpyright",
        purpose: "live Python errors and completion",
        need: Need::Recommended,
        found: pyright.map(|p| p.display().to_string()),
        install,
        auto,
    });

    deps.push(Dep {
        name: "gdb 14+",
        purpose: "debugging Rust (F7/F8, breakpoints)",
        need: Need::Recommended,
        found: crate::dap::find_gdb().map(|(_, version)| version),
        install: match os {
            Os::Windows => "winget install BrechtSanders.WinLibs.POSIX.MSVCRT".into(),
            Os::Debian => "sudo apt install gdb".into(),
            // RHEL 9's own gdb is too old for DAP; gcc-toolset has a newer one.
            Os::RedHat => "sudo dnf install gdb   (RHEL 9: sudo dnf install gcc-toolset-14-gdb)".into(),
            _ => "install gdb 14 or newer with your package manager".into(),
        },
        auto: (os == Os::Windows).then(|| {
            words("winget install --id BrechtSanders.WinLibs.POSIX.MSVCRT -e --accept-source-agreements --accept-package-agreements")
        }),
    });

    let (install, auto) = match os {
        // pip --user is blocked for the system Python on Ubuntu (PEP 668).
        Os::Debian => ("sudo apt install python3-debugpy".to_string(), None),
        _ => {
            let command = format!("{python} -m pip install --user debugpy");
            (command.clone(), Some(words(&command)))
        }
    };
    deps.push(Dep {
        name: "debugpy",
        purpose: "debugging Python (F7/F8, breakpoints)",
        need: Need::Recommended,
        found: crate::dap::debugpy_python(&here).map(|p| p.display().to_string()),
        install,
        auto,
    });

    if os != Os::Windows {
        let portal = ["/usr/libexec/xdg-desktop-portal", "/usr/lib/xdg-desktop-portal"]
            .iter()
            .find(|p| Path::new(p).is_file());
        deps.push(Dep {
            name: "xdg-desktop-portal",
            purpose: "Open/Save file dialogs",
            need: Need::Recommended,
            found: portal.map(|p| p.to_string()),
            install: match os {
                Os::Debian => "sudo apt install xdg-desktop-portal xdg-desktop-portal-gtk".into(),
                Os::RedHat => "sudo dnf install xdg-desktop-portal xdg-desktop-portal-gtk".into(),
                _ => "install xdg-desktop-portal with your package manager".into(),
            },
            auto: None,
        });
        deps.push(Dep {
            name: "Terminal emulator",
            purpose: "Run > Run in external terminal",
            need: Need::Optional,
            found: platform::linux_terminal().map(|(path, _)| path.display().to_string()),
            install: match os {
                Os::Debian => "sudo apt install gnome-terminal".into(),
                Os::RedHat => "sudo dnf install gnome-terminal".into(),
                _ => "install xterm or another terminal".into(),
            },
            auto: None,
        });
    }

    deps
}

/// Runs a dependency's `auto` install command; returns its last output line.
pub fn install(command: &[String]) -> Result<String, String> {
    let (program, args) = command.split_first().ok_or("empty command")?;
    let path = platform::find_program(program).unwrap_or_else(|| program.into());
    let output = platform::hidden_command(path)
        .args(args)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let last = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    if output.status.success() { Ok(last) } else { Err(last) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_lists_core_tools() {
        let list = check(Os::detect());
        for dep in &list {
            println!("{:<22} {:?}", dep.name, dep.found);
        }
        let names: Vec<_> = list.iter().map(|d| d.name).collect();
        assert!(names.contains(&"Rust toolchain"));
        assert!(names.contains(&"Python 3"));
        // This machine builds the IDE, so cargo must be found.
        assert!(list.iter().any(|d| d.name == "Rust toolchain" && d.found.is_some()));
    }
}
