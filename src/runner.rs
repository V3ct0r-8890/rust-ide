//! Build/check and run backend: drives cargo, rustc and python invisibly.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const RUST_GUI_CRATES: &[&str] = &[
    "iced", "eframe", "egui", "slint", "winit", "tauri", "fltk", "gtk", "druid", "xilem",
];
const PYTHON_GUI_MODULES: &[&str] = &[
    "tkinter", "PySide2", "PySide6", "PyQt5", "PyQt6", "pygame", "wx", "kivy", "flet",
];

/// Checks the source with `ast.parse` and prints the first syntax error as JSON.
const PYTHON_CHECK: &str = r#"
import ast, json, sys
path = sys.argv[1]
try:
    with open(path, encoding="utf-8-sig") as f:
        ast.parse(f.read(), path)
except SyntaxError as e:
    print(json.dumps({"line": e.lineno or 1, "col": e.offset or 1, "msg": e.msg}))
    sys.exit(1)
"#;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub file: Option<PathBuf>,
    pub line: usize,
    pub column: usize,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct RunTarget {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub gui: bool,
}

#[derive(Debug, Clone)]
pub struct BuildResult {
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub target: Option<RunTarget>,
}

impl BuildResult {
    fn failed(message: impl Into<String>) -> Self {
        Self {
            success: false,
            diagnostics: vec![Diagnostic {
                file: None,
                line: 0,
                column: 0,
                level: "error".into(),
                message: message.into(),
            }],
            target: None,
        }
    }
}

/// Compiles (Rust) or syntax-checks (Python) `file`.
pub fn build(file: &Path) -> BuildResult {
    match file.extension().and_then(|e| e.to_str()) {
        Some("rs") => match find_cargo_manifest(file) {
            Some(manifest) => build_cargo(&manifest),
            None => build_rustc(file),
        },
        Some("py" | "pyw") => check_python(file),
        _ => BuildResult::failed("Only .rs and .py files can be compiled"),
    }
}

/// Launches a built program: console programs get their own console window
/// that stays open, GUI programs are started directly.
pub fn run(target: &RunTarget) -> std::io::Result<()> {
    let mut command = launch_command(target);
    command.current_dir(&target.cwd).spawn().map(|_| ())
}

#[cfg(windows)]
fn launch_command(target: &RunTarget) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    if target.gui {
        let mut command = Command::new(&target.program);
        command.args(&target.args).creation_flags(CREATE_NO_WINDOW);
        return command;
    }

    let mut line = quote(&target.program.to_string_lossy());
    for arg in &target.args {
        line.push(' ');
        line.push_str(&quote(arg));
    }
    let mut command = Command::new("cmd.exe");
    command
        .raw_arg(format!(
            "/v:on /c \"{line} & echo. & echo [Process exited with code !errorlevel!] & pause\""
        ))
        .creation_flags(CREATE_NEW_CONSOLE);
    command
}

#[cfg(not(windows))]
fn launch_command(target: &RunTarget) -> Command {
    let mut command = Command::new(&target.program);
    command.args(&target.args);
    command
}

#[cfg(windows)]
fn quote(s: &str) -> String {
    format!("\"{s}\"")
}

fn find_cargo_manifest(file: &Path) -> Option<PathBuf> {
    file.ancestors()
        .skip(1)
        .map(|dir| dir.join("Cargo.toml"))
        .find(|manifest| manifest.is_file())
}

fn build_cargo(manifest: &Path) -> BuildResult {
    let dir = manifest.parent().unwrap_or(Path::new("."));
    let output = match hidden(Command::new("cargo"))
        .args(["build", "--message-format=json"])
        .current_dir(dir)
        .output()
    {
        Ok(output) => output,
        Err(e) => return BuildResult::failed(format!("Cannot start cargo: {e}")),
    };

    let mut diagnostics = Vec::new();
    let mut executable = None;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(json) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match json["reason"].as_str() {
            Some("compiler-message") => {
                diagnostics.extend(parse_rustc_diagnostic(&json["message"], dir));
            }
            Some("compiler-artifact") => {
                if let Some(exe) = json["executable"].as_str() {
                    executable = Some(PathBuf::from(exe));
                }
            }
            _ => {}
        }
    }

    let success = output.status.success();
    if !success && !diagnostics.iter().any(|d| d.level == "error") {
        // Failures that are not compiler messages (bad Cargo.toml, network, ...).
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .find(|l| l.trim_start().starts_with("error"))
            .unwrap_or("cargo build failed");
        diagnostics.push(BuildResult::failed(reason.trim()).diagnostics.remove(0));
    }

    let manifest_text = std::fs::read_to_string(manifest).unwrap_or_default();
    let gui = RUST_GUI_CRATES
        .iter()
        .any(|krate| manifest_text.lines().any(|l| l.trim_start().starts_with(krate)));

    BuildResult {
        success,
        diagnostics,
        target: executable.filter(|_| success).map(|program| RunTarget {
            program,
            args: Vec::new(),
            cwd: dir.to_path_buf(),
            gui,
        }),
    }
}

fn build_rustc(file: &Path) -> BuildResult {
    let dir = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    let out_dir = std::env::temp_dir().join("rust-ide");
    let _ = std::fs::create_dir_all(&out_dir);
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("main");
    let exe = out_dir.join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));

    let output = match hidden(Command::new("rustc"))
        .args(["--edition", "2024", "--error-format=json"])
        .arg(file)
        .arg("-o")
        .arg(&exe)
        .current_dir(&dir)
        .output()
    {
        Ok(output) => output,
        Err(e) => return BuildResult::failed(format!("Cannot start rustc: {e}")),
    };

    let diagnostics = String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|json| parse_rustc_diagnostic(&json, &dir))
        .collect();

    let source = std::fs::read_to_string(file).unwrap_or_default();
    let gui = RUST_GUI_CRATES
        .iter()
        .any(|krate| source.contains(&format!("use {krate}")));

    let success = output.status.success();
    BuildResult {
        success,
        diagnostics,
        target: success.then(|| RunTarget {
            program: exe,
            args: Vec::new(),
            cwd: dir,
            gui,
        }),
    }
}

/// Converts one rustc JSON diagnostic into a [`Diagnostic`], skipping notes
/// and summaries like "aborting due to previous error".
fn parse_rustc_diagnostic(message: &Value, base: &Path) -> Option<Diagnostic> {
    let level = message["level"].as_str()?;
    if level != "error" && level != "warning" {
        return None;
    }
    let text = message["message"].as_str()?.to_string();
    let span = message["spans"]
        .as_array()
        .and_then(|spans| spans.iter().find(|s| s["is_primary"].as_bool() == Some(true)));
    if span.is_none() && (text.starts_with("aborting due to") || text.contains("warning emitted"))
    {
        return None;
    }

    Some(Diagnostic {
        file: span.and_then(|s| s["file_name"].as_str()).map(|f| base.join(f)),
        line: span.and_then(|s| s["line_start"].as_u64()).unwrap_or(0) as usize,
        column: span.and_then(|s| s["column_start"].as_u64()).unwrap_or(0) as usize,
        level: level.to_string(),
        message: text,
    })
}

fn check_python(file: &Path) -> BuildResult {
    let dir = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    let output = match hidden(Command::new("python"))
        .args(["-c", PYTHON_CHECK])
        .arg(file)
        .current_dir(&dir)
        .output()
    {
        Ok(output) => output,
        Err(e) => return BuildResult::failed(format!("Cannot start python: {e}")),
    };

    let source = std::fs::read_to_string(file).unwrap_or_default();
    let gui = file.extension().is_some_and(|e| e == "pyw")
        || PYTHON_GUI_MODULES.iter().any(|module| {
            source.lines().any(|l| {
                let l = l.trim_start();
                l.starts_with(&format!("import {module}")) || l.starts_with(&format!("from {module}"))
            })
        });

    if output.status.success() {
        let program = if gui && cfg!(windows) { "pythonw" } else { "python" };
        return BuildResult {
            success: true,
            diagnostics: Vec::new(),
            target: Some(RunTarget {
                program: PathBuf::from(program),
                args: vec![file.to_string_lossy().into_owned()],
                cwd: dir,
                gui,
            }),
        };
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    match serde_json::from_str::<Value>(stdout.trim()) {
        Ok(json) => BuildResult {
            success: false,
            diagnostics: vec![Diagnostic {
                file: Some(file.to_path_buf()),
                line: json["line"].as_u64().unwrap_or(1) as usize,
                column: json["col"].as_u64().unwrap_or(1) as usize,
                level: "error".into(),
                message: json["msg"].as_str().unwrap_or("syntax error").to_string(),
            }],
            target: None,
        },
        Err(_) => BuildResult::failed(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    }
}

/// Keeps background tools from flashing a console window.
fn hidden(mut command: Command) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str, source: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("rust-ide-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }

    #[test]
    fn rust_error_points_at_line() {
        let file = sample("bad.rs", "fn main() {\n    let x: i32 = \"no\";\n}\n");
        let result = build(&file);
        assert!(!result.success);
        let error = result.diagnostics.iter().find(|d| d.level == "error").unwrap();
        assert_eq!(error.line, 2);
        assert!(error.file.as_deref().unwrap().ends_with("bad.rs"));
    }

    #[test]
    fn rust_success_gives_console_target() {
        let file = sample("good.rs", "fn main() { println!(\"hi\"); }\n");
        let result = build(&file);
        assert!(result.success, "{:?}", result.diagnostics);
        let target = result.target.unwrap();
        assert!(target.program.is_file());
        assert!(!target.gui);
    }

    #[test]
    fn python_syntax_error_points_at_line() {
        let file = sample("bad.py", "print('ok')\nif True\n    pass\n");
        let result = build(&file);
        assert!(!result.success);
        assert_eq!(result.diagnostics[0].line, 2);
    }

    #[test]
    fn python_with_bom_compiles() {
        let file = sample("bom.py", "\u{feff}print('hi')\n");
        assert!(build(&file).success);
    }

    #[test]
    fn python_tkinter_is_gui() {
        let file = sample("gui.py", "import tkinter\n");
        let result = build(&file);
        assert!(result.success, "{:?}", result.diagnostics);
        assert!(result.target.unwrap().gui);
    }
}
