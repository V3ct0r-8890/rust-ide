//! Help for installs that fail because of a firewall, a proxy or no internet: recognising the
//! failure, and step-by-step manual instructions that work with files fetched on another
//! machine. The text is shown in Tools > Dependencies (key M), so it needs no web access.

use crate::platform::Os;

/// Folder checked first for files the IDE would download (`RUST_IDE_OFFLINE_DIR`).
pub const OFFLINE_DIR_VAR: &str = "RUST_IDE_OFFLINE_DIR";

/// True when the error text looks like a network, proxy or certificate problem.
pub fn is_network_error(text: &str) -> bool {
    let text = text.to_lowercase();
    [
        "could not resolve",
        "couldn't resolve",
        "name or service not known",
        "temporary failure in name resolution",
        "connection refused",
        "connection timed out",
        "timed out",
        "network is unreachable",
        "no route to host",
        "failed to connect",
        "couldn't connect",
        "connection reset",
        "ssl",
        "tls handshake",
        "certificate",
        "proxy",
        "proxy authentication",
        "403 forbidden",
        "could not connect",
        "unable to connect",
        "failed to fetch",
        "download of",
        "cannot run curl",
        "network error",
        "no such host",
        "unreachable",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

/// What to show when installing `name` failed with `error`.
pub fn failure_message(name: &str, error: &str) -> String {
    if is_network_error(error) {
        format!(
            "{name} failed: {error}\nThis looks like a network, proxy or firewall problem. Press M for manual steps \
             (or set HTTPS_PROXY and try again)."
        )
    } else {
        format!("{name} failed: {error}\nPress M for manual steps.")
    }
}

const PROXY: &str = "Behind a proxy: set HTTPS_PROXY (and HTTP_PROXY), e.g. http://user:pass@proxy:3128, before starting \
the IDE or the install command. A company certificate: set CURL_CA_BUNDLE (curl, pip: PIP_CERT, rustup: SSL_CERT_FILE) \
to the .pem file.";

/// Manual / offline instructions for one dependency.
pub fn manual_steps(name: &str, os: Os) -> String {
    let body = match name {
        "Managed Python" => {
            let (file, hint) = match os {
                Os::Windows => ("cpython-3.13.N+TAG-x86_64-pc-windows-msvc-install_only.tar.gz", ""),
                Os::MacOs => ("cpython-3.13.N+TAG-aarch64-apple-darwin-install_only.tar.gz", ""),
                _ => ("cpython-3.13.N+TAG-x86_64-unknown-linux-gnu-install_only.tar.gz", ""),
            };
            format!(
                "On a computer with internet, open https://github.com/astral-sh/python-build-standalone/releases/latest and \
                 download SHA256SUMS and {file}{hint}. Copy both files into one folder on this machine, set \
                 {OFFLINE_DIR_VAR} to that folder, restart the IDE and press Install again. The checksum is still verified."
            )
        }
        "Managed Rust" => {
            let (url, file) = match os {
                Os::Windows => (
                    "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe",
                    "rustup-init.exe and rustup-init.exe.sha256",
                ),
                Os::MacOs => (
                    "https://static.rust-lang.org/rustup/dist/aarch64-apple-darwin/rustup-init",
                    "rustup-init and rustup-init.sha256",
                ),
                _ => (
                    "https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init",
                    "rustup-init and rustup-init.sha256",
                ),
            };
            format!(
                "Download {file} (from {url}, add .sha256 for the checksum) on a computer with internet and put them in one \
                 folder here; set {OFFLINE_DIR_VAR} to it and press Install. rustup-init then still needs \
                 static.rust-lang.org for the compiler itself: allow that host, set RUSTUP_DIST_SERVER and \
                 RUSTUP_UPDATE_ROOT to your company mirror, or install Rust from an offline installer at \
                 https://forge.rust-lang.org/infra/other-installation-methods.html (standalone installers)."
            )
        }
        "Rust toolchain" => match os {
            Os::Windows => "Download rustup-init.exe from https://rustup.rs on another computer, run it, and choose the \
                 stable-x86_64-pc-windows-gnu host. Offline: use the standalone .msi installer for x86_64-pc-windows-gnu from \
                 https://forge.rust-lang.org/infra/other-installation-methods.html. Company mirror: set RUSTUP_DIST_SERVER and \
                 RUSTUP_UPDATE_ROOT first."
                .to_string(),
            _ => "Download the standalone installer tarball for your system from \
                 https://forge.rust-lang.org/infra/other-installation-methods.html, unpack it and run ./install.sh. Or fetch \
                 rustup-init from https://static.rust-lang.org/rustup/dist/<target>/rustup-init, copy it here and run it \
                 (company mirror: set RUSTUP_DIST_SERVER and RUSTUP_UPDATE_ROOT first)."
                .to_string(),
        },
        "MinGW-w64 (gcc, dlltool)" | "C compiler / linker" => match os {
            Os::Windows => "Download the WinLibs zip (POSIX threads, UCRT or MSVCRT, x86_64) from \
                 https://github.com/brechtsanders/winlibs_mingw/releases, unzip it (for example to C:\\mingw64), and add \
                 C:\\mingw64\\bin to the Path (Settings > System > About > Advanced system settings > Environment Variables). \
                 This also provides gdb."
                .to_string(),
            Os::Debian => "Copy the .deb files from a machine with internet (apt download build-essential and its \
                 dependencies), or use your company apt mirror, then sudo apt install ./*.deb."
                .to_string(),
            Os::RedHat => "Download the rpm packages on another machine (dnf download --resolve gcc) or use your company \
                 mirror, then sudo dnf install ./*.rpm."
                .to_string(),
            Os::MacOs => "xcode-select --install needs Apple's servers; or download the Command Line Tools from \
                 https://developer.apple.com/download/all/ on another Mac."
                .to_string(),
            Os::OtherLinux => "Install gcc and binutils with your distribution's tools or an internal mirror.".to_string(),
        },
        "Python 3" => match os {
            Os::Windows => "Download the Python 3.13 installer from https://www.python.org/downloads/ on another computer, \
                 copy it here and run it (tick Add python.exe to PATH). Or use Managed Python with the offline folder."
                .to_string(),
            Os::Debian => "sudo apt install python3 from your mirror, or copy the .deb files (apt download python3).".to_string(),
            Os::RedHat => "sudo dnf install python3 from your mirror, or copy the rpm files (dnf download --resolve python3).".to_string(),
            _ => "Install Python 3 from your system's package source, or use Managed Python with the offline folder.".to_string(),
        },
        "rust-analyzer" => "Needs rustup: rustup component add rust-analyzer. Offline: download rust-analyzer-<target>.gz from \
             https://github.com/rust-lang/rust-analyzer/releases, gunzip it, rename it rust-analyzer(.exe) and put it in a \
             folder on PATH."
            .to_string(),
        "basedpyright" | "debugpy" => format!(
            "On a computer with internet and the same Python version: pip download {name} -d wheels. Copy the wheels folder \
             here, then: pip install --no-index --find-links wheels {name}. Or point pip at your company index: pip install \
             --index-url https://your-mirror/simple {name} (permanent: pip config set global.index-url ...)."
        ),
        "gdb 14+" => match os {
            Os::Windows => "gdb comes with WinLibs: download the zip from \
                 https://github.com/brechtsanders/winlibs_mingw/releases (any 2024 or newer release has gdb 14+), unzip it and \
                 add its bin folder to Path. Check with gdb --version in PowerShell (not Git Bash)."
                .to_string(),
            Os::Debian => "Ubuntu 24.04 has gdb 15: sudo apt install gdb (from your mirror). Older releases: copy a gdb 14+ \
                 .deb from packages.ubuntu.com for your release, or build from https://ftp.gnu.org/gnu/gdb/ (needs \
                 libgmp-dev libmpfr-dev python3-dev, ./configure --with-python=python3 && make && sudo make install)."
                .to_string(),
            Os::RedHat => "RHEL / Rocky / Alma 9: sudo dnf install gcc-toolset-14-gdb (from AppStream or your mirror; the IDE \
                 finds it in /opt/rh). Fedora and RHEL 10 already have gdb 14+. Offline: dnf download --resolve \
                 gcc-toolset-14-gdb on a connected machine and install the rpms here."
                .to_string(),
            _ => "Install gdb 14 or newer from your package source, or build it from https://ftp.gnu.org/gnu/gdb/.".to_string(),
        },
        ".NET SDK 10" => "Download the SDK installer or tarball from https://dotnet.microsoft.com/download on another \
             computer and run it here. Package feeds: dotnet-sdk-10.0 from your mirror."
            .to_string(),
        "csharp-ls" => "dotnet tool install -g csharp-ls needs nuget.org. Offline: on a connected machine run dotnet tool \
             install csharp-ls --tool-path tools, copy the folder here, or use --add-source with a local folder of the .nupkg."
            .to_string(),
        "netcoredbg" => "Download netcoredbg-win64.zip (Linux: netcoredbg-linux-amd64.tar.gz) from \
             https://github.com/Samsung/netcoredbg/releases, unpack it and put the folder on PATH."
            .to_string(),
        _ => "Use your system's package manager with an internal mirror, or download the package on a computer with internet \
             and copy it here. The install command shown for this row is what the IDE would run."
            .to_string(),
    };
    format!("Manual install - {name}:\n{body}\n{PROXY}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_failures_are_recognised() {
        for text in [
            "curl: (6) Could not resolve host: static.rust-lang.org",
            "download of https://x failed: Connection timed out",
            "curl: (60) SSL certificate problem: unable to get local issuer certificate",
            "HTTP/1.1 407 Proxy Authentication Required",
        ] {
            assert!(is_network_error(text), "{text}");
        }
        assert!(!is_network_error("checksum mismatch for python.tar.gz"));
        assert!(!is_network_error("E: Unable to locate package gdbx"));
    }

    #[test]
    fn the_failure_message_points_to_manual_steps() {
        let network = failure_message("Managed Rust", "download of https://x failed: Could not resolve host");
        assert!(network.contains("firewall") && network.contains("Press M"));
        let other = failure_message("gdb 14+", "E: Unable to locate package");
        assert!(!other.contains("firewall") && other.contains("Press M"));
    }

    #[test]
    fn every_dependency_has_steps_for_every_system() {
        let names = [
            "Rust toolchain", "MinGW-w64 (gcc, dlltool)", "C compiler / linker", "Python 3", ".NET SDK 10", "csharp-ls",
            "Managed Python", "Managed Rust", "rust-analyzer", "basedpyright", "gdb 14+", "netcoredbg", "debugpy", "other",
        ];
        for os in [Os::Windows, Os::Debian, Os::RedHat, Os::OtherLinux, Os::MacOs] {
            for name in names {
                let text = manual_steps(name, os);
                assert!(text.starts_with("Manual install") && text.contains("HTTPS_PROXY"), "{name}");
                assert!(text.len() > 150, "{name} has real instructions");
            }
        }
        assert!(manual_steps("Managed Python", Os::Windows).contains(OFFLINE_DIR_VAR));
    }
}
