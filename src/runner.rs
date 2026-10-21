//! Build/check and run backend: drives cargo, rustc, python and dotnet invisibly.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::platform;

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

#[derive(Debug, Clone, PartialEq)]
pub struct RunTarget {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub gui: bool,
    /// The Rust function the debugger stops in first (`crate::main`).
    pub entry: Option<String>,
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

/// Compiles (Rust, C#) or syntax-checks (Python) `file`.
pub fn build(file: &Path) -> BuildResult {
    match file.extension().and_then(|e| e.to_str()) {
        Some("rs") => match find_cargo_manifest(file) {
            Some(manifest) => build_cargo(&manifest, false),
            None => build_rustc(file, false),
        },
        Some("py" | "pyw") => check_python(file),
        Some("cs") => build_csharp(file),
        _ => BuildResult::failed("Only .rs, .py and .cs files can be compiled"),
    }
}

/// Looks for errors without producing a program: `cargo check` for Rust
/// projects, `rustc --emit=metadata` for single Rust files (both skip code
/// generation, so they are much faster than a build), and the syntax parse
/// for Python. C# has no cheaper check than its build (the compiler is the
/// checker), so it builds. The result never has a run target.
pub fn check(file: &Path) -> BuildResult {
    let mut result = match file.extension().and_then(|e| e.to_str()) {
        Some("rs") => match find_cargo_manifest(file) {
            Some(manifest) => build_cargo(&manifest, true),
            None => build_rustc(file, true),
        },
        Some("py" | "pyw") => check_python(file),
        Some("cs") => build_csharp(file),
        _ => BuildResult::failed("Only .rs, .py and .cs files can be checked"),
    };
    result.target = None;
    result
}

/// Launches a built program outside the IDE: console programs get their own
/// terminal window that stays open, GUI programs are started directly.
pub fn run_external(target: &RunTarget) -> Result<(), String> {
    let mut command = if target.gui {
        let mut command = platform::hidden_command(&target.program);
        command.args(&target.args);
        command
    } else {
        platform::external_terminal(&target.program, &target.args)?
    };
    crate::env::apply(&mut command, &target.cwd);
    command
        .current_dir(&target.cwd)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cannot run {}: {e}", target.program.display()))
}

/// Starts a built program with stdin/stdout/stderr piped to the IDE.
pub fn spawn_captured(target: &RunTarget) -> std::io::Result<Child> {
    let mut command = platform::hidden_command(&target.program);
    let is_python = target
        .program
        .file_stem()
        .is_some_and(|s| s.to_string_lossy().starts_with("python"));
    if is_python {
        command.arg("-u"); // unbuffered, so output appears as it is printed
    }
    crate::env::apply(&mut command, &target.cwd);
    command
        .args(&target.args)
        .current_dir(&target.cwd)
        .env("DOTNET_NOLOGO", "1")
        .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("TERM", "dumb")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// A tool command, resolved even when its directory is missing from PATH.
fn tool(name: &str) -> Command {
    platform::hidden_command(platform::find_program(name).unwrap_or_else(|| PathBuf::from(name)))
}

/// The `Cargo.toml` of the package this file belongs to. Only files in the
/// package's source folders count: a loose `.rs` file elsewhere under a
/// project (in `target/`, `docs/`, ...) is compiled on its own.
pub fn find_cargo_manifest(file: &Path) -> Option<PathBuf> {
    file.ancestors().skip(1).find_map(|dir| {
        let manifest = dir.join("Cargo.toml");
        if !manifest.is_file() {
            return None;
        }
        let first = file.strip_prefix(dir).ok()?.components().next()?;
        let first = first.as_os_str().to_str()?;
        ["src", "examples", "tests", "benches", "build.rs"].contains(&first).then_some(manifest)
    })
}

fn build_cargo(manifest: &Path, check_only: bool) -> BuildResult {
    let dir = manifest.parent().unwrap_or(Path::new("."));
    let verb = if check_only { "check" } else { "build" };
    let mut cargo = tool("cargo");
    crate::env::apply(&mut cargo, dir);
    let output = match cargo
        .args([verb, "--message-format=json"])
        .current_dir(dir)
        .output()
    {
        Ok(output) => output,
        Err(e) => return BuildResult::failed(format!("Cannot start cargo: {e}")),
    };

    let mut diagnostics = Vec::new();
    let mut executable = None;
    let mut entry = None;
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
                    entry = json["target"]["name"].as_str().map(|name| format!("{}::main", name.replace('-', "_")));
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
            .map_or_else(|| format!("cargo {verb} failed"), str::to_string);
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
            entry,
        }),
    }
}

fn build_rustc(file: &Path, check_only: bool) -> BuildResult {
    let dir = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    let out_dir = std::env::temp_dir().join("rust-ide");
    let _ = std::fs::create_dir_all(&out_dir);
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("main");
    let exe = out_dir.join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    let metadata = out_dir.join(format!("{stem}.rmeta"));

    let mut rustc = tool("rustc");
    crate::env::apply(&mut rustc, &dir);
    rustc.args(["--edition", "2024", "--error-format=json"]);
    if check_only {
        // Type-check only: no machine code is generated.
        rustc.arg("--emit=metadata").arg("-o").arg(&metadata);
    } else {
        rustc.arg("-g").arg("-o").arg(&exe); // -g: debug info for the debugger
    }
    let output = match rustc
        .arg(file)
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
            entry: Some(format!("{}::main", stem.replace('-', "_"))),
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

/// The project a C# file belongs to: the nearest folder above it with a `.csproj`.
pub fn find_csproj(file: &Path) -> Option<PathBuf> {
    file.ancestors().skip(1).find_map(|dir| {
        std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "csproj") && p.is_file())
    })
}

/// Builds a C# file with the .NET SDK: its project when it has one, else the
/// file alone (a file-based app, .NET 10 and newer).
fn build_csharp(file: &Path) -> BuildResult {
    let project = find_csproj(file);
    let dir = match &project {
        Some(p) => p.parent().unwrap_or(Path::new(".")).to_path_buf(),
        None => file.parent().unwrap_or(Path::new(".")).to_path_buf(),
    };
    let subject = project.as_deref().unwrap_or(file);
    let mut dotnet = tool("dotnet");
    crate::env::apply(&mut dotnet, &dir);
    let output = match dotnet
        .args(["build", "--nologo", "-consoleloggerparameters:NoSummary"])
        .arg(subject)
        .env("DOTNET_NOLOGO", "1")
        .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
        .env("DOTNET_CLI_UI_LANGUAGE", "en")
        .current_dir(&dir)
        .output()
    {
        Ok(output) => output,
        Err(e) => return BuildResult::failed(format!("Cannot start dotnet (is the .NET SDK installed?): {e}")),
    };

    let text = format!("{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for diagnostic in text.lines().filter_map(|l| parse_msbuild_line(l, &dir)) {
        // MSBuild repeats a message once per project and again in its summary.
        let same = |d: &Diagnostic| {
            (&d.file, d.line, d.column, &d.message) == (&diagnostic.file, diagnostic.line, diagnostic.column, &diagnostic.message)
        };
        if !diagnostics.iter().any(same) {
            diagnostics.push(diagnostic);
        }
    }

    let success = output.status.success();
    if !success && !diagnostics.iter().any(|d| d.level == "error") {
        // Failures that are not compiler messages (restore, missing SDK, bad project).
        let reason = text
            .lines()
            .find(|l| l.contains("error "))
            .map_or_else(|| "dotnet build failed".to_string(), |l| l.trim().to_string());
        diagnostics.push(BuildResult::failed(reason).diagnostics.remove(0));
    }

    let dotnet_path = platform::find_program("dotnet").unwrap_or_else(|| PathBuf::from("dotnet"));
    let args = match &project {
        Some(p) => vec!["run".to_string(), "--no-build".into(), "--project".into(), p.to_string_lossy().into_owned()],
        None => vec!["run".to_string(), file.to_string_lossy().into_owned()],
    };
    BuildResult {
        success,
        diagnostics,
        target: success.then(|| RunTarget { program: dotnet_path, args, cwd: dir, gui: false, entry: None }),
    }
}

/// One MSBuild/Roslyn message: `path(line,col): error CS0103: text [project]`.
fn parse_msbuild_line(line: &str, base: &Path) -> Option<Diagnostic> {
    let line = line.trim();
    let open = line.find("): ")?;
    let (head, rest) = (&line[..open], &line[open + 3..]);
    let paren = head.rfind('(')?;
    let (path, position) = (&head[..paren], &head[paren + 1..]);
    let mut numbers = position.split(',').map(|n| n.trim().parse::<usize>());
    let (row, column) = (numbers.next()?.ok()?, numbers.next()?.ok()?);
    let (level, message) = rest.split_once(' ')?;
    if level != "error" && level != "warning" {
        return None;
    }
    // Drop the trailing "[C:\...\app.csproj]" MSBuild adds.
    let message = match message.rfind(" [") {
        Some(i) if message.ends_with(']') => &message[..i],
        _ => message,
    };
    Some(Diagnostic {
        file: Some(base.join(path)),
        line: row,
        column,
        level: level.to_string(),
        message: message.trim().to_string(),
    })
}

fn check_python(file: &Path) -> BuildResult {
    let dir = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    let venv = crate::lsp::venv_python(&dir);
    let mut python = match &venv {
        Some(path) => platform::hidden_command(path),
        None => tool(platform::python()),
    };
    crate::env::apply(&mut python, &dir);
    let output = match python
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
        let program = if gui && cfg!(windows) { "pythonw" } else { platform::python() };
        // The project's .venv interpreter wins over the system one.
        let program = match venv {
            Some(path) if gui && cfg!(windows) => path.with_file_name("pythonw.exe"),
            Some(path) => path,
            None => platform::find_program(program).unwrap_or_else(|| PathBuf::from(program)),
        };
        return BuildResult {
            success: true,
            diagnostics: Vec::new(),
            target: Some(RunTarget {
                program,
                args: vec![file.to_string_lossy().into_owned()],
                cwd: dir,
                gui,
                entry: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msbuild_messages_become_diagnostics() {
        let base = Path::new("C:\\proj");
        let line = "C:\\proj\\Program.cs(3,9): error CS0029: Cannot convert type 'string' to 'int' [C:\\proj\\proj.csproj]";
        let d = parse_msbuild_line(line, base).unwrap();
        assert_eq!((d.line, d.column, d.level.as_str()), (3, 9, "error"));
        assert_eq!(d.message, "CS0029: Cannot convert type 'string' to 'int'");
        // A Windows path is one component on Linux and macOS, so compare the text.
        assert!(d.file.unwrap().to_string_lossy().ends_with("Program.cs"));
        let w = parse_msbuild_line("a.cs(1,2,1,5): warning CS0168: The variable 'x' is declared but never used", base).unwrap();
        assert_eq!((w.line, w.column, w.level.as_str()), (1, 2, "warning"));
        assert!(parse_msbuild_line("Build FAILED.", base).is_none());
        assert!(parse_msbuild_line("  Restored a.csproj (in 364 ms).", base).is_none());
        assert!(parse_msbuild_line("a.cs(1,2): info CS0000: x", base).is_none());
    }

    #[test]
    #[ignore = "needs the .NET SDK"]
    fn csharp_files_and_projects_build_and_run() {
        let root = std::env::temp_dir().join("rust-ide-tests").join("csharp-build");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let bad = root.join("bad.cs");
        std::fs::write(&bad, "int x = \"no\";\nConsole.WriteLine(y);\n").unwrap();
        let result = check(&bad);
        assert!(!result.success && result.target.is_none());
        let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.level == "error").collect();
        assert_eq!(errors.len(), 2, "{:?}", result.diagnostics);
        assert_eq!(errors[0].line, 1);
        assert!(errors[0].message.starts_with("CS0029"));

        let good = root.join("good.cs");
        std::fs::write(&good, "Console.WriteLine(\"hello cs\");\n").unwrap();
        let result = build(&good);
        assert!(result.success, "{:?}", result.diagnostics);
        let target = result.target.expect("a run target");
        let out = Command::new(&target.program).args(&target.args).current_dir(&target.cwd).output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("hello cs"));
        assert!(check(&good).target.is_none());

        let project = root.join("app");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(
            project.join("app.csproj"),
            "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>\n",
        )
        .unwrap();
        let program = project.join("Program.cs");
        std::fs::write(&program, "Console.WriteLine(\"from project\");\n").unwrap();
        assert_eq!(find_csproj(&program), Some(project.join("app.csproj")));
        let result = build(&program);
        assert!(result.success, "{:?}", result.diagnostics);
        let target = result.target.unwrap();
        let out = Command::new(&target.program).args(&target.args).current_dir(&target.cwd).output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("from project"), "{out:?}");
    }
    fn sample(name: &str, source: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("rust-ide-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }

    fn temp_exe(stem: &str) -> PathBuf {
        std::env::temp_dir().join("rust-ide").join(format!("{stem}{}", std::env::consts::EXE_SUFFIX))
    }

    #[test]
    fn check_rust_file_finds_errors_without_building() {
        let _ = std::fs::remove_file(temp_exe("chk_bad"));
        let file = sample("chk_bad.rs", "fn main() {\n    let x: i32 = \"no\";\n}\n");
        let result = check(&file);
        assert!(!result.success);
        let error = result.diagnostics.iter().find(|d| d.level == "error").unwrap();
        assert_eq!(error.line, 2);
        assert!(result.target.is_none());
        assert!(!temp_exe("chk_bad").exists(), "a check must not produce a program");
    }

    #[test]
    fn check_rust_file_succeeds_without_a_run_target() {
        let _ = std::fs::remove_file(temp_exe("chk_good"));
        let file = sample("chk_good.rs", "fn main() { println!(\"hi\"); }\n");
        let result = check(&file);
        assert!(result.success, "{:?}", result.diagnostics);
        assert!(result.target.is_none());
        assert!(!temp_exe("chk_good").exists());
        assert!(build(&file).target.is_some(), "a build still gives a run target");
    }

    #[test]
    fn check_cargo_project_reports_errors_and_builds_nothing() {
        let root = std::env::temp_dir().join("rust-ide-tests").join("check-project");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"chk\"\nversion = \"0.1.0\"\nedition = \"2024\"\n").unwrap();
        let main = root.join("src").join("main.rs");

        std::fs::write(&main, "fn main() {\n    let n: u8 = \"x\";\n}\n").unwrap();
        let result = check(&main);
        assert!(!result.success);
        let error = result.diagnostics.iter().find(|d| d.level == "error").unwrap();
        assert_eq!(error.line, 2);
        assert!(error.file.as_deref().is_some_and(|f| f.ends_with("main.rs")));

        std::fs::write(&main, "fn main() { println!(\"ok\"); }\n").unwrap();
        let result = check(&main);
        assert!(result.success, "{:?}", result.diagnostics);
        assert!(result.target.is_none());
        let exe = root.join("target").join("debug").join(format!("chk{}", std::env::consts::EXE_SUFFIX));
        assert!(!exe.exists(), "cargo check must not link a program");
    }

    #[test]
    fn check_python_uses_the_syntax_parse() {
        let bad = sample("chk_bad.py", "print('ok')\nif True\n    pass\n");
        let result = check(&bad);
        assert!(!result.success);
        assert_eq!(result.diagnostics[0].line, 2);
        let good = sample("chk_good.py", "print('ok')\n");
        let result = check(&good);
        assert!(result.success);
        assert!(result.target.is_none());
        assert!(!check(Path::new("notes.txt")).success, "other file types are refused");
    }

    #[test]
    fn only_source_folders_belong_to_a_cargo_project() {
        let root = std::env::temp_dir().join("rust-ide-tests").join("project");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]
name = \"p\"
").unwrap();
        assert!(find_cargo_manifest(&root.join("src").join("main.rs")).is_some());
        assert!(find_cargo_manifest(&root.join("examples").join("demo.rs")).is_some());
        assert!(find_cargo_manifest(&root.join("target").join("e2e").join("x.rs")).is_none());
        assert!(find_cargo_manifest(&root.join("notes.rs")).is_none());
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
