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
        Some(dir) => section("Project", Some(project_path(dir)), &project(dir), &mut out),
        None => out.push_str("\n# Project: open a file of a project to see its environment\n"),
    }
    out
}

/// What a value would be after the layers are put together (used by tests and the summary).
pub fn merged(dir: Option<&Path>) -> HashMap<String, String> {
    effective(dir).into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
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
}
