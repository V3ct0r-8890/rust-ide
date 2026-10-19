//! Environment variables for the programs the IDE starts, in two groups:
//!
//! 1. the **general** environment (proxy settings, `PATH` additions, tool
//!    folders): one file next to the IDE's settings, `environment.env`, used for
//!    every project and every tool the IDE starts;
//! 2. the **project** environment: `.rust-ide/env` in the project folder, used
//!    for the programs of that project (builds, runs, the debugger).
//!
//! Both are plain `NAME=value` files (`#` comments, optional `export `,
//! optional quotes). A value may use `$NAME`, `${NAME}` or `%NAME%`, so
//! `PATH=C:\tools;${PATH}` adds to the existing path. A project value wins over
//! a general one with the same name, and both win over the IDE's own
//! environment. The files are read each time something is started, so edits
//! apply at once.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::platform;

/// A variable name compares without case on Windows only.
fn key(name: &str) -> String {
    if cfg!(windows) { name.to_uppercase() } else { name.to_string() }
}

/// Where the general environment is kept.
pub fn general_path() -> Option<PathBuf> {
    if let Some(file) = std::env::var_os("RUST_IDE_GENERAL_ENV_FILE") {
        return Some(PathBuf::from(file));
    }
    Some(platform::config_path()?.with_file_name("environment.env"))
}

/// The project folder a file belongs to: the folder that has `.rust-ide/env`,
/// else the Git root, else the nearest folder with a project file, else the
/// file's own folder.
pub fn project_root(dir: &Path) -> PathBuf {
    let ancestors: Vec<&Path> = dir.ancestors().collect();
    if let Some(found) = ancestors.iter().find(|d| d.join(".rust-ide").join("env").is_file()) {
        return found.to_path_buf();
    }
    if let Some(found) = ancestors.iter().find(|d| d.join(".git").exists()) {
        return found.to_path_buf();
    }
    const MARKERS: [&str; 5] = ["Cargo.toml", "pyproject.toml", "requirements.txt", ".venv", ".rust-ide"];
    ancestors
        .iter()
        .find(|d| MARKERS.iter().any(|m| d.join(m).exists()))
        .map_or_else(|| dir.to_path_buf(), |d| d.to_path_buf())
}

/// The project environment file of the project `dir` belongs to (it may not exist yet).
pub fn project_path(dir: &Path) -> PathBuf {
    project_root(dir).join(".rust-ide").join("env")
}

/// Expands `$NAME`, `${NAME}` and `%NAME%` using `lookup`; unknown names stay as written.
fn expand(value: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let reference = match c {
            '$' if chars.get(i + 1) == Some(&'{') => {
                chars[i + 2..].iter().position(|&x| x == '}').map(|end| (i + 2 + end + 1, chars[i + 2..i + 2 + end].iter().collect::<String>()))
            }
            '$' => {
                let len = chars[i + 1..].iter().take_while(|x| x.is_ascii_alphanumeric() || **x == '_').count();
                (len > 0).then(|| (i + 1 + len, chars[i + 1..i + 1 + len].iter().collect::<String>()))
            }
            '%' => chars[i + 1..]
                .iter()
                .position(|&x| x == '%')
                .filter(|&end| end > 0 && chars[i + 1..i + 1 + end].iter().all(|x| x.is_ascii_alphanumeric() || *x == '_'))
                .map(|end| (i + 1 + end + 1, chars[i + 1..i + 1 + end].iter().collect::<String>())),
            _ => None,
        };
        match reference {
            Some((next, name)) => {
                match lookup(&name) {
                    Some(v) => out.push_str(&v),
                    None => out.extend(&chars[i..next]),
                }
                i = next;
            }
            None => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Parses `NAME=value` lines. Variables defined earlier in the same text can be
/// used by later ones; `base` supplies everything else.
pub fn parse(text: &str, base: &dyn Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    let mut vars: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((name, value)) = line.split_once('=') else { continue };
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
            continue;
        }
        let mut value = value.trim();
        // Quotes around the whole value are removed; a trailing " # comment" after
        // an unquoted value is too.
        let quoted = value.len() >= 2
            && ((value.starts_with('"') && value.ends_with('"')) || (value.starts_with('\'') && value.ends_with('\'')));
        if quoted {
            value = &value[1..value.len() - 1];
        } else if let Some(at) = value.find(" #") {
            value = value[..at].trim_end();
        }
        let expanded = expand(value, &|n| {
            vars.iter().rev().find(|(k, _)| key(k) == key(n)).map(|(_, v)| v.clone()).or_else(|| base(n))
        });
        vars.retain(|(k, _)| key(k) != key(name));
        vars.push((name.to_string(), expanded));
    }
    vars
}

fn process_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// The general environment (an empty list when there is no file).
pub fn general() -> Vec<(String, String)> {
    let Some(text) = general_path().and_then(|p| std::fs::read_to_string(p).ok()) else { return Vec::new() };
    parse(&text, &process_var)
}

/// The environment of the project `dir` belongs to, with the general one as its base.
pub fn project(dir: &Path) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(project_path(dir)) else { return Vec::new() };
    let general = general();
    parse(&text, &|n| general.iter().find(|(k, _)| key(k) == key(n)).map(|(_, v)| v.clone()).or_else(|| process_var(n)))
}

/// Everything to add to a program started for the project of `dir`: general
/// variables, then project ones (which win).
pub fn effective(dir: Option<&Path>) -> Vec<(String, String)> {
    let mut vars = general();
    if let Some(dir) = dir {
        for (name, value) in project(dir) {
            vars.retain(|(k, _)| key(k) != key(&name));
            vars.push((name, value));
        }
    }
    vars
}

/// Adds the project environment (and the general one) to a command that works in `dir`.
pub fn apply(command: &mut Command, dir: &Path) {
    command.envs(effective(Some(dir)));
}

/// Sets up the Python environment of the project `dir` belongs to: creates
/// `.venv` in the project folder when there is none, then installs
/// `requirements.txt` (or the project itself, when it has a `pyproject.toml`
/// or `setup.py`). Slow: run it off the UI thread. Returns a one-line summary.
pub fn python_setup(dir: &Path) -> Result<String, String> {
    let root = project_root(dir);
    let mut done = Vec::new();
    let venv_python = match crate::lsp::venv_python(&root) {
        Some(python) => python,
        None => {
            let system = platform::find_program(platform::python())
                .ok_or_else(|| "Python is not installed (see Options > Dependencies)".to_string())?;
            let mut command = platform::hidden_command(system);
            apply(&mut command, &root);
            let output = command
                .args(["-m", "venv", ".venv"])
                .current_dir(&root)
                .output()
                .map_err(|e| format!("Cannot start Python: {e}"))?;
            if !output.status.success() {
                return Err(last_line("Creating .venv failed", &output));
            }
            done.push("created .venv");
            crate::lsp::venv_python(&root).ok_or_else(|| "The new .venv has no Python".to_string())?
        }
    };
    let install: Option<Vec<&str>> = if root.join("requirements.txt").is_file() {
        Some(vec!["-m", "pip", "install", "-r", "requirements.txt"])
    } else if root.join("pyproject.toml").is_file() || root.join("setup.py").is_file() {
        Some(vec!["-m", "pip", "install", "-e", "."])
    } else {
        None
    };
    if let Some(args) = install {
        let mut command = platform::hidden_command(&venv_python);
        apply(&mut command, &root);
        let output = command
            .args(args)
            .current_dir(&root)
            .output()
            .map_err(|e| format!("Cannot start pip: {e}"))?;
        if !output.status.success() {
            return Err(last_line("pip install failed", &output));
        }
        done.push("installed the requirements");
    }
    if done.is_empty() {
        done.push("the .venv already exists and there is nothing to install");
    }
    Ok(format!("Python environment of {}: {}", root.display(), done.join(", ")))
}

fn last_line(what: &str, output: &std::process::Output) -> String {
    let text = format!("{}{}", String::from_utf8_lossy(&output.stderr), String::from_utf8_lossy(&output.stdout));
    let line = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
    format!("{what}: {}", line.trim())
}

const GENERAL_TEMPLATE: &str = "\
# General environment of Rust IDE: used for every project and every tool the IDE starts.
# One NAME=value per line. $NAME, ${NAME} and %NAME% use existing values.
# Remove the # to switch a line on. Saving (F2) is enough; no restart is needed.
#
# Proxy (builds, cargo, pip and downloads):
# HTTP_PROXY=http://proxy.example.com:8080
# HTTPS_PROXY=http://proxy.example.com:8080
# NO_PROXY=localhost,127.0.0.1
#
# Add folders to the search path:
# PATH=${PATH}
";

const PROJECT_TEMPLATE: &str = "\
# Environment of this project: used when Rust IDE builds, runs or debugs it.
# One NAME=value per line; a value here wins over the general environment.
# Do not commit secrets: the IDE keeps this folder out of Git (.gitignore).
#
# RUST_LOG=debug
# DATABASE_URL=postgres://localhost/dev
";

/// Creates the general environment file if it is missing; returns its path.
pub fn ensure_general_file() -> std::io::Result<PathBuf> {
    let path = general_path().ok_or_else(|| std::io::Error::other("no config directory"))?;
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, GENERAL_TEMPLATE)?;
    }
    Ok(path)
}

/// Creates the project environment file if it is missing (and keeps `.rust-ide/`
/// out of Git when the project has a `.gitignore`); returns its path.
pub fn ensure_project_file(dir: &Path) -> std::io::Result<PathBuf> {
    let path = project_path(dir);
    if !path.exists() {
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        std::fs::write(&path, PROJECT_TEMPLATE)?;
        let root = project_root(dir);
        let ignore = root.join(".gitignore");
        if let Ok(text) = std::fs::read_to_string(&ignore) {
            if !text.lines().any(|l| matches!(l.trim(), ".rust-ide" | ".rust-ide/" | "/.rust-ide" | "/.rust-ide/")) {
                let sep = if text.ends_with('\n') || text.is_empty() { "" } else { "\n" };
                std::fs::write(&ignore, format!("{text}{sep}.rust-ide/\n"))?;
            }
        }
    }
    Ok(path)
}

/// A readable summary: where each group comes from and the variables of both.
pub fn describe(dir: Option<&Path>) -> String {
    let mut out = String::from("# Environment for programs started by Rust IDE\n");
    let section = |title: &str, path: Option<PathBuf>, vars: &[(String, String)], out: &mut String| {
        out.push('\n');
        match path {
            Some(p) if p.is_file() => out.push_str(&format!("# {title}: {}\n", p.display())),
            Some(p) => out.push_str(&format!("# {title}: {} (not created yet)\n", p.display())),
            None => out.push_str(&format!("# {title}: none\n")),
        }
        for (name, value) in vars {
            out.push_str(&format!("{name}={value}\n"));
        }
    };
    section("General", general_path(), &general(), &mut out);
    match dir {
        Some(dir) => {
            section("Project", Some(project_path(dir)), &project(dir), &mut out);
            out.push_str(&match rust_toolchain(dir) {
                Some((name, from)) => format!("\n# Rust toolchain: {name} (from {from})\n"),
                None => "\n# Rust toolchain: the rustup default (set one in Options > Rust toolchain)\n".to_string(),
            });
        }
        None => out.push_str("\n# Project: open a file of a project to see its environment\n"),
    }
    out
}

/// What a value would be after the layers are put together (used by tests and the summary).
pub fn merged(dir: Option<&Path>) -> HashMap<String, String> {
    effective(dir).into_iter().collect()
}

/// The Rust toolchain a project builds with and where that choice comes from:
/// `RUSTUP_TOOLCHAIN` in the environment files, else the nearest
/// `rust-toolchain.toml` / `rust-toolchain`. `None` means rustup's default.
pub fn rust_toolchain(dir: &Path) -> Option<(String, String)> {
    let vars = effective(Some(dir));
    if let Some((_, value)) = vars.iter().find(|(k, _)| key(k) == key("RUSTUP_TOOLCHAIN")) {
        if !value.trim().is_empty() {
            return Some((value.trim().to_string(), "RUSTUP_TOOLCHAIN in the environment".to_string()));
        }
    }
    dir.ancestors().find_map(|d| {
        let toml = d.join("rust-toolchain.toml");
        let plain = d.join("rust-toolchain");
        let channel = std::fs::read_to_string(&toml)
            .ok()
            .and_then(|text| toolchain_channel(&text, true))
            .map(|c| (c, "rust-toolchain.toml"))
            .or_else(|| {
                let text = std::fs::read_to_string(&plain).ok()?;
                // The old file is a bare channel name; a TOML one may also use this name.
                toolchain_channel(&text, text.contains('=')).map(|c| (c, "rust-toolchain"))
            })?;
        Some((channel.0, channel.1.to_string()))
    })
}

/// The channel named by a toolchain file's text.
fn toolchain_channel(text: &str, toml: bool) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !toml {
            return Some(line.to_string());
        }
        if let Some(rest) = line.strip_prefix("channel") {
            let value = rest.trim_start().strip_prefix('=')?.trim();
            let value = value.split('#').next().unwrap_or("").trim().trim_matches(|c| c == '"' || c == '\'');
            return (!value.is_empty()).then(|| value.to_string());
        }
    }
    None
}

/// Whether `name` can be a toolchain name (`stable`, `1.85.0`, `nightly-2025-01-01`,
/// `stable-x86_64-pc-windows-gnu`).
pub fn valid_toolchain(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Sets (or, with `None`, removes) one variable in the project environment file,
/// creating the file when needed. Other lines are kept as they are.
pub fn set_project_var(dir: &Path, name: &str, value: Option<&str>) -> std::io::Result<PathBuf> {
    let path = ensure_project_file(dir)?;
    let text = std::fs::read_to_string(&path)?;
    let mut lines: Vec<String> = Vec::new();
    let mut done = false;
    for line in text.lines() {
        let body = line.trim().strip_prefix("export ").unwrap_or(line.trim());
        let is_it = body.split_once('=').is_some_and(|(n, _)| key(n.trim()) == key(name));
        if !is_it {
            lines.push(line.to_string());
        } else if let (false, Some(value)) = (done, value) {
            lines.push(format!("{name}={value}"));
            done = true;
        }
    }
    if let (false, Some(value)) = (done, value) {
        lines.push(format!("{name}={value}"));
    }
    std::fs::write(&path, lines.join("\n") + "\n")?;
    Ok(path)
}

/// A bare channel (`stable`, `nightly-2025-01-01`, `1.85.0`) names the MSVC
/// toolchain on a Windows machine whose rustup defaults to MSVC, which needs
/// Visual Studio. On Windows the IDE uses the GNU one, so the host is added.
pub fn qualify_toolchain(name: &str, windows: bool) -> String {
    let has_host = ["x86_64", "aarch64", "i686"].iter().any(|h| name.contains(h));
    let channel = name.starts_with("stable") || name.starts_with("beta") || name.starts_with("nightly") || name.starts_with(|c: char| c.is_ascii_digit());
    if windows && channel && !has_host { format!("{name}-x86_64-pc-windows-gnu") } else { name.to_string() }
}

/// One line of the toolchain list: what to store, a hint for the user, and
/// whether it can build here.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolchainChoice {
    /// The `RUSTUP_TOOLCHAIN` value; empty is the rustup default.
    pub name: String,
    pub note: String,
    pub usable: bool,
    pub current: bool,
}

/// The names in the output of `rustup toolchain list`.
fn parse_toolchain_list(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("no installed"))
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// The installed toolchains (slow: starts `rustup`; run it off the UI thread).
pub fn installed_toolchains() -> Vec<String> {
    let Some(rustup) = platform::find_program("rustup") else { return Vec::new() };
    let output = platform::hidden_command(rustup).args(["toolchain", "list"]).output();
    output.map(|o| parse_toolchain_list(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default()
}

/// A hint about a toolchain; `usable` is false when it cannot link on this machine.
pub fn toolchain_note(name: &str, windows: bool, has_link: bool, has_gcc: bool) -> (String, bool) {
    if windows && name.contains("windows-msvc") {
        return if has_link {
            ("MSVC: uses Visual Studio's linker".to_string(), true)
        } else {
            ("MSVC: needs Visual Studio's link.exe, which is not installed".to_string(), false)
        };
    }
    if windows && name.contains("windows-gnu") {
        return if has_gcc {
            ("GNU: links with MinGW (recommended on Windows)".to_string(), true)
        } else {
            ("GNU: MinGW gcc was not found (see Options > Dependencies)".to_string(), false)
        };
    }
    let kind = if name.starts_with("stable") {
        "stable release"
    } else if name.starts_with("beta") {
        "beta release"
    } else if name.starts_with("nightly") {
        "nightly: newest features, can break"
    } else if name.starts_with(|c: char| c.is_ascii_digit()) {
        "a pinned version"
    } else {
        "a custom toolchain"
    };
    (kind.to_string(), true)
}

/// The list for the picker: the default first, then every installed toolchain.
pub fn toolchain_choices(installed: &[String], dir: &Path) -> Vec<ToolchainChoice> {
    let chosen = effective(Some(dir))
        .into_iter()
        .find(|(k, _)| key(k) == key("RUSTUP_TOOLCHAIN"))
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_default();
    let (has_link, has_gcc) = (platform::find_program("link").is_some(), platform::find_program("gcc").is_some());
    let mut choices = vec![ToolchainChoice {
        name: String::new(),
        note: "rustup's default (or the project's rust-toolchain.toml)".to_string(),
        usable: true,
        current: chosen.is_empty(),
    }];
    for name in installed {
        let (note, usable) = toolchain_note(name, cfg!(windows), has_link, has_gcc);
        choices.push(ToolchainChoice { name: name.clone(), note, usable, current: *name == chosen });
    }
    // A choice stored earlier that is no longer installed stays visible.
    if !chosen.is_empty() && !installed.contains(&chosen) {
        choices.push(ToolchainChoice { name: chosen, note: "not installed (rustup toolchain install ...)".to_string(), usable: false, current: true });
    }
    choices
}

/// Chooses the Rust toolchain of the project (`None` goes back to the default).
pub fn set_rust_toolchain(dir: &Path, name: Option<&str>) -> Result<String, String> {
    if let Some(name) = name {
        if !valid_toolchain(name) {
            return Err(format!("\"{name}\" is not a toolchain name (examples: stable, nightly, 1.85.0)"));
        }
    }
    let qualified = name.map(|n| qualify_toolchain(n, cfg!(windows)));
    let name = qualified.as_deref();
    set_project_var(dir, "RUSTUP_TOOLCHAIN", name).map_err(|e| format!("Cannot write the project environment: {e}"))?;
    Ok(match name {
        Some(name) => format!("Rust toolchain of {}: {name}", project_root(dir).display()),
        None => "Rust toolchain: back to the default (or rust-toolchain.toml)".to_string(),
    })
}

/// The toolchain as the status line shows it for a Rust file: `Rust nightly`.
pub fn toolchain_label(dir: &Path) -> String {
    rust_toolchain(dir).map_or_else(|| "Rust default".to_string(), |(name, _)| format!("Rust {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that point `RUST_IDE_GENERAL_ENV_FILE` somewhere take turns.
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn none(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn parses_lines_comments_quotes_and_export() {
        let vars = parse(
            "# comment\n\nA=1\nexport B = two words \nC=\"quoted # kept\"\nD='single'\nE=plain # trailing\nbad line\n=novalue\nF.G=x\n",
            &none,
        );
        let get = |n: &str| vars.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str());
        assert_eq!(get("A"), Some("1"));
        assert_eq!(get("B"), Some("two words"));
        assert_eq!(get("C"), Some("quoted # kept"));
        assert_eq!(get("D"), Some("single"));
        assert_eq!(get("E"), Some("plain"));
        assert_eq!(get("F.G"), Some("x"));
        assert_eq!(vars.len(), 6, "lines without a name or an = are skipped");
    }

    #[test]
    fn references_use_earlier_lines_then_the_base() {
        let base = |n: &str| (n == "HOME_DIR").then(|| "/home/me".to_string());
        let vars = parse("ROOT=${HOME_DIR}/p\nBIN=$ROOT/bin\nWIN=%ROOT%\\x\nMISSING=${NOPE}-$NOPE2-%NOPE3%\nPRICE=5$\n", &base);
        let get = |n: &str| vars.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone()).unwrap();
        assert_eq!(get("ROOT"), "/home/me/p");
        assert_eq!(get("BIN"), "/home/me/p/bin");
        assert_eq!(get("WIN"), "/home/me/p\\x");
        assert_eq!(get("MISSING"), "${NOPE}-$NOPE2-%NOPE3%", "unknown names stay as written");
        assert_eq!(get("PRICE"), "5$");
    }

    #[test]
    fn a_later_line_replaces_an_earlier_one_and_can_extend_itself() {
        let base = |n: &str| (n == "PATH").then(|| "/usr/bin".to_string());
        let vars = parse("PATH=/opt/a:${PATH}\nPATH=/opt/b:${PATH}\nX=1\nX=2\n", &base);
        assert_eq!(vars.iter().filter(|(k, _)| k == "PATH").count(), 1);
        assert_eq!(vars.iter().find(|(k, _)| k == "PATH").unwrap().1, "/opt/b:/opt/a:/usr/bin");
        assert_eq!(vars.iter().find(|(k, _)| k == "X").unwrap().1, "2");
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("rust-ide-env-test").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_project_root_is_found_from_a_nested_folder() {
        let root = scratch("root");
        let nested = root.join("crates").join("cli").join("src");
        std::fs::create_dir_all(&nested).unwrap();
        // Nothing marks a project: the folder itself.
        assert_eq!(project_root(&nested), nested);
        std::fs::write(root.join("crates").join("cli").join("Cargo.toml"), "").unwrap();
        assert_eq!(project_root(&nested), root.join("crates").join("cli"), "nearest project file");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        assert_eq!(project_root(&nested), root, "the Git root wins over a nested project file");
        std::fs::create_dir_all(root.join("crates").join(".rust-ide")).unwrap();
        std::fs::write(root.join("crates").join(".rust-ide").join("env"), "A=1\n").unwrap();
        assert_eq!(project_root(&nested), root.join("crates"), "an existing environment file wins");
    }

    #[test]
    fn project_variables_win_over_general_ones_and_are_applied() {
        let root = scratch("layers");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        // SAFETY: only tests in this module use this variable (they take turns below).
        let _guard = TURN.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("RUST_IDE_GENERAL_ENV_FILE", root.join("environment.env")) };
        std::fs::write(
            root.join("environment.env"),
            "HTTP_PROXY=http://proxy:8080\nSHARED=general\nTOOLS=/opt/tools\n",
        )
        .unwrap();
        assert_eq!(general_path().unwrap(), root.join("environment.env"));

        // No project file: general only.
        let merged_general = merged(Some(&root));
        assert_eq!(merged_general.get("SHARED").map(String::as_str), Some("general"));

        let project_file = ensure_project_file(&root).unwrap();
        assert_eq!(project_file, root.join(".rust-ide").join("env"));
        std::fs::write(&project_file, "SHARED=project\nBIN=${TOOLS}/bin\nONLY_HERE=1\n").unwrap();
        let all = merged(Some(&root));
        assert_eq!(all.get("SHARED").map(String::as_str), Some("project"));
        assert_eq!(all.get("BIN").map(String::as_str), Some("/opt/tools/bin"), "a project line can use a general variable");
        assert_eq!(all.get("HTTP_PROXY").map(String::as_str), Some("http://proxy:8080"));
        assert!(!merged(None).contains_key("ONLY_HERE"), "without a project only the general ones apply");

        let mut command = Command::new("tool");
        apply(&mut command, &root);
        let envs: HashMap<String, String> = command
            .get_envs()
            .filter_map(|(k, v)| Some((k.to_string_lossy().into_owned(), v?.to_string_lossy().into_owned())))
            .collect();
        assert_eq!(envs.get("ONLY_HERE").map(String::as_str), Some("1"));
        assert_eq!(envs.get("SHARED").map(String::as_str), Some("project"));

        // hidden_command carries the general environment on its own.
        let general_only = platform::hidden_command("tool");
        assert!(general_only.get_envs().any(|(k, v)| k == "HTTP_PROXY" && v.is_some()));

        let text = describe(Some(&root));
        assert!(text.contains("# General:") && text.contains("# Project:") && text.contains("ONLY_HERE=1"));
        unsafe { std::env::remove_var("RUST_IDE_GENERAL_ENV_FILE") };
    }

    #[test]
    fn a_new_project_file_is_kept_out_of_git() {
        let root = scratch("ignore");
        std::fs::write(root.join("Cargo.toml"), "").unwrap();
        std::fs::write(root.join(".gitignore"), "/target").unwrap();
        ensure_project_file(&root).unwrap();
        assert_eq!(std::fs::read_to_string(root.join(".gitignore")).unwrap(), "/target\n.rust-ide/\n");
        // Asking again changes nothing.
        ensure_project_file(&root).unwrap();
        assert_eq!(std::fs::read_to_string(root.join(".gitignore")).unwrap().matches(".rust-ide").count(), 1);
        // No .gitignore: none is invented.
        let other = scratch("ignore2");
        std::fs::write(other.join("Cargo.toml"), "").unwrap();
        ensure_project_file(&other).unwrap();
        assert!(!other.join(".gitignore").exists());
    }

    #[test]
    fn the_toolchain_comes_from_the_environment_or_the_toolchain_file() {
        let _guard = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let root = scratch("toolchain");
        unsafe { std::env::set_var("RUST_IDE_GENERAL_ENV_FILE", root.join("none.env")) };
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        assert_eq!(rust_toolchain(&src), None);
        assert_eq!(toolchain_label(&src), "Rust default");

        std::fs::write(root.join("rust-toolchain.toml"), "[toolchain]\nchannel = \"nightly-2025-01-01\" # pinned\ncomponents = [\"clippy\"]\n").unwrap();
        assert_eq!(rust_toolchain(&src), Some(("nightly-2025-01-01".into(), "rust-toolchain.toml".into())));
        std::fs::remove_file(root.join("rust-toolchain.toml")).unwrap();
        std::fs::write(root.join("rust-toolchain"), "1.85.0\n").unwrap();
        assert_eq!(rust_toolchain(&src), Some(("1.85.0".into(), "rust-toolchain".into())));

        // The IDE's own choice wins over the file, keeps other lines and can be removed.
        set_project_var(&root, "KEEP", Some("1")).unwrap();
        let beta = qualify_toolchain("beta", cfg!(windows));
        let nightly = qualify_toolchain("nightly", cfg!(windows));
        set_rust_toolchain(&root, Some("beta")).unwrap();
        assert_eq!(rust_toolchain(&src), Some((beta.clone(), "RUSTUP_TOOLCHAIN in the environment".into())));
        assert_eq!(toolchain_label(&src), format!("Rust {beta}"));
        set_rust_toolchain(&root, Some("nightly")).unwrap();
        let file = std::fs::read_to_string(project_path(&root)).unwrap();
        assert_eq!(file.matches("RUSTUP_TOOLCHAIN=").count(), 1, "{file}");
        assert!(file.contains("KEEP=1") && file.contains(&format!("RUSTUP_TOOLCHAIN={nightly}")), "{file}");
        assert!(describe(Some(&src)).contains(&format!("# Rust toolchain: {nightly}")));
        // The picker marks the stored choice and keeps one that is not installed.
        let choices = toolchain_choices(&["stable-x86_64-pc-windows-gnu".to_string()], &src);
        assert!(!choices[0].current && choices[0].name.is_empty(), "the default is not the current one");
        assert!(choices.iter().any(|c| c.name == nightly && c.current && !c.usable), "{choices:?}");
        set_rust_toolchain(&root, None).unwrap();
        assert!(!std::fs::read_to_string(project_path(&root)).unwrap().contains("RUSTUP_TOOLCHAIN="));
        assert_eq!(rust_toolchain(&src).unwrap().0, "1.85.0");

        assert!(set_rust_toolchain(&root, Some("bad name; x")).is_err());
        assert!(valid_toolchain("stable-x86_64-pc-windows-gnu") && !valid_toolchain(""));
        unsafe { std::env::remove_var("RUST_IDE_GENERAL_ENV_FILE") };
    }

    #[test]
    fn toolchain_names_lists_and_notes() {
        let list = "stable-x86_64-pc-windows-gnu (default)\nnightly-x86_64-pc-windows-msvc\n\n1.85.0-x86_64-pc-windows-gnu (active)\n";
        assert_eq!(
            parse_toolchain_list(list),
            ["stable-x86_64-pc-windows-gnu", "nightly-x86_64-pc-windows-msvc", "1.85.0-x86_64-pc-windows-gnu"]
        );
        assert!(parse_toolchain_list("no installed toolchains\n").is_empty());

        assert_eq!(qualify_toolchain("stable", true), "stable-x86_64-pc-windows-gnu");
        assert_eq!(qualify_toolchain("nightly-2025-01-01", true), "nightly-2025-01-01-x86_64-pc-windows-gnu");
        assert_eq!(qualify_toolchain("1.85.0", true), "1.85.0-x86_64-pc-windows-gnu");
        assert_eq!(qualify_toolchain("stable-x86_64-pc-windows-msvc", true), "stable-x86_64-pc-windows-msvc", "a full name is kept");
        assert_eq!(qualify_toolchain("stable", false), "stable", "only Windows adds the host");
        assert_eq!(qualify_toolchain("my-custom", true), "my-custom");

        assert!(!toolchain_note("stable-x86_64-pc-windows-msvc", true, false, true).1, "no link.exe");
        assert!(toolchain_note("stable-x86_64-pc-windows-msvc", true, true, true).1);
        assert!(toolchain_note("stable-x86_64-pc-windows-gnu", true, false, true).0.contains("recommended"));
        assert!(!toolchain_note("stable-x86_64-pc-windows-gnu", true, false, false).1, "no gcc");
        assert!(toolchain_note("nightly-x86_64-unknown-linux-gnu", false, false, true).0.contains("nightly"));
        assert!(toolchain_note("1.85.0-x86_64-unknown-linux-gnu", false, false, true).0.contains("pinned"));
    }

    #[test]
    #[ignore = "needs Python with venv; run with --ignored"]
    fn python_setup_creates_a_venv_and_installs_requirements() {
        let root = scratch("venv");
        std::fs::write(root.join("requirements.txt"), "# nothing to install
").unwrap();
        let message = python_setup(&root).unwrap();
        assert!(message.contains("created .venv") && message.contains("installed"), "{message}");
        assert!(crate::lsp::venv_python(&root).unwrap().is_file());
        std::fs::write(root.join("hello.py"), "print('hi')
").unwrap();
        let built = crate::runner::build(&root.join("hello.py"));
        let program = built.target.expect("a run target").program;
        assert_eq!(program, crate::lsp::venv_python(&root).unwrap(), "runs with the project's interpreter");
        let again = python_setup(&root).unwrap();
        assert!(!again.contains("created"), "{again}");
    }
}
