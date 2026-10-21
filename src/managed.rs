//! Tools the IDE downloads for itself ("Set up tools for the IDE"): a relocatable Python
//! (python-build-standalone) and Rust through rustup, kept in the IDE's own data folder, apart
//! from the system. Opt-in: nothing is downloaded until the user presses Install in
//! Tools > Dependencies. Managed tools are looked up before the ones on PATH.

use std::path::{Path, PathBuf};

use crate::platform;

const PYTHON_RELEASES: &str = "https://github.com/astral-sh/python-build-standalone/releases";
/// Python minor series to install.
const PYTHON_SERIES: &str = "cpython-3.13.";

/// The pseudo command stored in `Dep::auto`; `deps::install` runs it itself.
pub const COMMAND: &str = "rust-ide:managed";

/// Where managed tools live: `RUST_IDE_TOOLS`, else the user's data folder.
pub fn root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("RUST_IDE_TOOLS") {
        return Some(PathBuf::from(dir));
    }
    let base = platform::data_base("LOCALAPPDATA", "XDG_DATA_HOME", &[".local", "share"])?;
    Some(base.join("rust-ide").join("tools"))
}

pub fn python_dir() -> Option<PathBuf> {
    root().map(|r| r.join("python"))
}

pub fn python_exe() -> Option<PathBuf> {
    let dir = python_dir()?;
    let exe = if cfg!(windows) { dir.join("python.exe") } else { dir.join("bin").join("python3") };
    exe.is_file().then_some(exe)
}

pub fn rustup_home() -> Option<PathBuf> {
    root().map(|r| r.join("rustup"))
}

pub fn cargo_home() -> Option<PathBuf> {
    root().map(|r| r.join("cargo"))
}

pub fn cargo_exe() -> Option<PathBuf> {
    platform::find_in(&[cargo_home()?.join("bin")], "cargo")
}

/// Folders of the installed managed tools, searched before PATH.
pub fn bin_dirs() -> Vec<PathBuf> {
    root().map(|r| bin_dirs_in(&r)).unwrap_or_default()
}

fn bin_dirs_in(root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let python = root.join("python");
    let python_exe = if cfg!(windows) { python.join("python.exe") } else { python.join("bin").join("python3") };
    if python_exe.is_file() {
        if cfg!(windows) {
            dirs.push(python.clone());
            dirs.push(python.join("Scripts"));
        } else {
            dirs.push(python.join("bin"));
        }
    }
    let cargo_bin = root.join("cargo").join("bin");
    if platform::find_in(std::slice::from_ref(&cargo_bin), "cargo").is_some() {
        dirs.push(cargo_bin);
    }
    dirs
}

/// Environment a managed program needs (the rustup proxies read these).
pub fn env_for(program: &Path) -> Vec<(&'static str, PathBuf)> {
    root().map(|r| env_for_in(&r, program)).unwrap_or_default()
}

fn env_for_in(root: &Path, program: &Path) -> Vec<(&'static str, PathBuf)> {
    let cargo = root.join("cargo");
    if !program.starts_with(&cargo) {
        return Vec::new();
    }
    vec![("CARGO_HOME", cargo), ("RUSTUP_HOME", root.join("rustup"))]
}

/// What to install, from the command stored in `Dep::auto`.
pub fn run(command: &[String]) -> Result<String, String> {
    match command.get(1).map(String::as_str) {
        Some("python") => install_python(),
        Some("rust") => install_rust(),
        other => Err(format!("unknown managed tool {other:?}")),
    }
}

// ----- Python -----

fn python_triple() -> &'static str {
    let arm = cfg!(target_arch = "aarch64");
    match (cfg!(windows), cfg!(target_os = "macos"), arm) {
        (true, _, false) => "x86_64-pc-windows-msvc",
        (true, _, true) => "aarch64-pc-windows-msvc",
        (_, true, false) => "x86_64-apple-darwin",
        (_, true, true) => "aarch64-apple-darwin",
        (_, _, false) => "x86_64-unknown-linux-gnu",
        (_, _, true) => "aarch64-unknown-linux-gnu",
    }
}

/// Picks the newest `cpython-3.13.N+TAG-<triple>-install_only.tar.gz` from a `SHA256SUMS` file:
/// the file name, its hash and the release tag.
pub fn pick_python(sums: &str, triple: &str) -> Option<(String, String, String)> {
    let suffix = format!("-{triple}-install_only.tar.gz");
    sums.lines()
        .filter_map(|line| {
            let (hash, name) = line.split_once(char::is_whitespace)?;
            let name = name.trim().trim_start_matches('*');
            let rest = name.strip_prefix(PYTHON_SERIES)?.strip_suffix(&suffix)?;
            let (patch, tag) = rest.split_once('+')?;
            Some((patch.parse::<u32>().ok()?, hash.to_string(), name.to_string(), tag.to_string()))
        })
        .max_by_key(|(patch, ..)| *patch)
        .map(|(_, hash, name, tag)| (name, hash, tag))
}

fn install_python() -> Result<String, String> {
    let root = root().ok_or("no data folder for managed tools")?;
    let work = root.join("download");
    reset_dir(&work)?;
    let sums_file = work.join("SHA256SUMS");
    download(&format!("{PYTHON_RELEASES}/latest/download/SHA256SUMS"), &sums_file)?;
    let sums = std::fs::read_to_string(&sums_file).map_err(|e| e.to_string())?;
    let (name, hash, tag) =
        pick_python(&sums, python_triple()).ok_or("no matching Python build in the release")?;
    let archive = work.join(&name);
    download(&format!("{PYTHON_RELEASES}/download/{tag}/{}", name.replace('+', "%2B")), &archive)?;
    verify(&archive, &hash)?;
    let target = python_dir().ok_or("no data folder")?;
    if target.exists() {
        std::fs::remove_dir_all(&target).map_err(|e| format!("cannot replace {}: {e}", target.display()))?;
    }
    extract(&archive, &root)?; // contains a top-level `python` folder
    let _ = std::fs::remove_dir_all(&work);
    let exe = python_exe().ok_or("Python was unpacked but python was not found")?;
    Ok(format!("Python installed: {}", exe.display()))
}

// ----- Rust -----

fn rustup_target() -> &'static str {
    let arm = cfg!(target_arch = "aarch64");
    match (cfg!(windows), cfg!(target_os = "macos"), arm) {
        (true, _, false) => "x86_64-pc-windows-gnu",
        (true, _, true) => "aarch64-pc-windows-msvc",
        (_, true, false) => "x86_64-apple-darwin",
        (_, true, true) => "aarch64-apple-darwin",
        (_, _, false) => "x86_64-unknown-linux-gnu",
        (_, _, true) => "aarch64-unknown-linux-gnu",
    }
}

/// The hash from a `.sha256` file (`<hash>` or `<hash> *<name>`).
pub fn first_word(text: &str) -> Option<String> {
    text.split_whitespace().next().map(str::to_string)
}

fn install_rust() -> Result<String, String> {
    let root = root().ok_or("no data folder for managed tools")?;
    let work = root.join("download");
    reset_dir(&work)?;
    let target = rustup_target();
    let file = if cfg!(windows) { "rustup-init.exe" } else { "rustup-init" };
    let url = format!("https://static.rust-lang.org/rustup/dist/{target}/{file}");
    let init = work.join(file);
    download(&url, &init)?;
    let hash_file = work.join("rustup-init.sha256");
    download(&format!("{url}.sha256"), &hash_file)?;
    let hash = first_word(&std::fs::read_to_string(&hash_file).map_err(|e| e.to_string())?)
        .ok_or("empty checksum file")?;
    verify(&init, &hash)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&init, std::fs::Permissions::from_mode(0o755));
    }
    let output = platform::hidden_command(&init)
        .args(["-y", "--no-modify-path", "--profile", "minimal", "--default-host", target])
        .args(["--default-toolchain", "stable", "-c", "rust-analyzer"])
        .env("RUSTUP_HOME", rustup_home().ok_or("no data folder")?)
        .env("CARGO_HOME", cargo_home().ok_or("no data folder")?)
        .output()
        .map_err(|e| format!("cannot run rustup-init: {e}"))?;
    if !output.status.success() {
        let text = String::from_utf8_lossy(&output.stderr);
        let last = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("rustup-init failed");
        return Err(last.trim().to_string());
    }
    let _ = std::fs::remove_dir_all(&work);
    let cargo = cargo_exe().ok_or("Rust was installed but cargo was not found")?;
    Ok(format!("Rust installed: {}", cargo.display()))
}

// ----- Helpers -----

fn reset_dir(dir: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))
}

fn download(url: &str, file: &Path) -> Result<(), String> {
    let output = platform::hidden_command("curl")
        .args(["--fail", "--silent", "--show-error", "--location", "--retry", "2", "--output"])
        .arg(file)
        .arg(url)
        .output()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&output.stderr);
    Err(format!("download of {url} failed: {}", text.trim()))
}

fn extract(archive: &Path, into: &Path) -> Result<(), String> {
    let output = platform::hidden_command("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(into)
        .output()
        .map_err(|e| format!("cannot run tar: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("tar failed: {}", String::from_utf8_lossy(&output.stderr).trim()))
    }
}

/// Parses the hash out of `certutil -hashfile` or `sha256sum` output.
pub fn parse_hash_output(text: &str) -> Option<String> {
    text.lines()
        .map(|l| l.split_whitespace().collect::<String>())
        .find(|l| l.len() >= 64 && l.is_char_boundary(64) && l[..64].chars().all(|c| c.is_ascii_hexdigit()))
        .map(|l| l[..64].to_lowercase())
}

fn sha256(file: &Path) -> Result<String, String> {
    let output = if cfg!(windows) {
        platform::hidden_command("certutil").arg("-hashfile").arg(file).arg("SHA256").output()
    } else {
        platform::hidden_command("sha256sum").arg(file).output()
    }
    .map_err(|e| format!("cannot compute checksum: {e}"))?;
    parse_hash_output(&String::from_utf8_lossy(&output.stdout)).ok_or_else(|| "cannot read the checksum".to_string())
}

fn verify(file: &Path, expected: &str) -> Result<(), String> {
    let actual = sha256(file)?;
    if actual.eq_ignore_ascii_case(expected.trim()) {
        Ok(())
    } else {
        Err(format!("checksum mismatch for {} (expected {expected}, got {actual})", file.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_python_for_the_platform_is_picked() {
        let sums = "\
aaa  cpython-3.13.9+20261013-x86_64-pc-windows-msvc-install_only.tar.gz
bbb  cpython-3.13.16+20261013-x86_64-pc-windows-msvc-install_only.tar.gz
ccc  cpython-3.13.16+20261013-x86_64-pc-windows-msvc-install_only_stripped.tar.gz
ddd  cpython-3.14.1+20261013-x86_64-pc-windows-msvc-install_only.tar.gz
eee  cpython-3.13.16+20261013-x86_64-unknown-linux-gnu-install_only.tar.gz
";
        let (name, hash, tag) = pick_python(sums, "x86_64-pc-windows-msvc").unwrap();
        assert_eq!(name, "cpython-3.13.16+20261013-x86_64-pc-windows-msvc-install_only.tar.gz");
        assert_eq!((hash.as_str(), tag.as_str()), ("bbb", "20261013"));
        assert!(pick_python(sums, "riscv64-unknown-linux-gnu").is_none());
    }

    #[test]
    fn checksum_output_is_parsed() {
        let hash = "6d5b5709addc0122c916d8c810da8d8a7b086a5d64fa805ef404d506392aadc8";
        let certutil = format!("SHA256 hash of x.exe:\r\n{}\r\nCertUtil: -hashfile command completed successfully.\r\n", hash);
        assert_eq!(parse_hash_output(&certutil).as_deref(), Some(hash));
        assert_eq!(parse_hash_output(&format!("{hash}  x.exe\n")).as_deref(), Some(hash));
        assert_eq!(first_word(&format!("{hash} *rustup-init.exe\n")).as_deref(), Some(hash));
    }

    #[test]
    fn managed_tools_come_first_and_set_their_environment() {
        let dir = std::env::temp_dir().join(format!("rust-ide-managed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(bin_dirs_in(&dir).is_empty(), "nothing installed yet");
        let bin = dir.join("cargo").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let name = if cfg!(windows) { "cargo.exe" } else { "cargo" };
        std::fs::write(bin.join(name), "").unwrap();
        assert_eq!(bin_dirs_in(&dir), [bin.clone()]);
        let env = env_for_in(&dir, &bin.join(name));
        assert!(env.iter().any(|(k, v)| *k == "CARGO_HOME" && *v == dir.join("cargo")));
        assert!(env.iter().any(|(k, v)| *k == "RUSTUP_HOME" && *v == dir.join("rustup")));
        assert!(env_for_in(&dir, Path::new("/usr/bin/cargo")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore = "downloads about 50 MB from github.com; run with --ignored"]
    fn python_installs_and_runs() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("managed-tools");
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the only test that sets the variable, and it is ignored by default.
        unsafe { std::env::set_var("RUST_IDE_TOOLS", &dir) };
        let message = install_python().expect("install");
        let exe = python_exe().expect("python.exe");
        let output = platform::hidden_command(&exe).args(["-c", "import venv, pip; print('ok')"]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ok", "{message}");
        assert!(bin_dirs().contains(&exe.parent().unwrap().to_path_buf()));
        unsafe { std::env::remove_var("RUST_IDE_TOOLS") };
    }
}