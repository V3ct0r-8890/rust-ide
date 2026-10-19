//! Debug Adapter Protocol client: drives `gdb -i dap` (Rust) and
//! `python -m debugpy.adapter` (Python) over stdio.
//!
//! One session per debugged program. The UI sends [`Command`]s through a
//! [`Handle`] and receives [`Event`]s from the stream returned by [`run`].

use std::collections::{BTreeMap, HashMap};
use std::io::BufReader;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::{Arc, Mutex};

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt, stream};
use serde_json::{Value, json};

use crate::lsp::{read_message, write_message};
use crate::platform;

/// Which adapter a session talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adapter {
    /// `gdb -i dap` (gdb 14 or newer), for Rust built with debug info.
    Gdb,
    /// `python -m debugpy.adapter`.
    Debugpy,
    /// `netcoredbg --interpreter=vscode` (Samsung, MIT), for C# on .NET.
    NetCoreDbg,
}

/// Everything needed to start a debug session.
#[derive(Debug, Clone)]
pub struct Launch {
    pub adapter: Adapter,
    /// The adapter command line (program first).
    pub command: Vec<String>,
    /// The `launch` request arguments.
    pub arguments: Value,
    /// Function to stop in first (Rust's `crate::main`), when stopping at entry.
    pub entry_function: Option<String>,
    /// Breakpoints: file -> 1-based line numbers.
    pub breakpoints: BTreeMap<PathBuf, Vec<usize>>,
    /// One-shot breakpoint (Go to cursor), removed at the next stop.
    pub temporary: Option<(PathBuf, usize)>,
}

#[derive(Debug, Clone)]
pub enum Command {
    Continue,
    Next,
    StepIn,
    StepOut,
    /// Replace the breakpoints of one file (1-based lines).
    SetBreakpoints { path: PathBuf, lines: Vec<usize> },
    /// Continue to a line once (Go to cursor).
    RunTo { path: PathBuf, line: usize },
    Evaluate { expression: String },
    Terminate,
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub name: String,
    pub file: Option<PathBuf>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub enum Event {
    Started(Handle),
    /// Execution stopped; `frames[0]` is where.
    Stopped { frames: Vec<Frame>, locals: Vec<Variable> },
    Running,
    Output(String),
    Evaluated { expression: String, value: String },
    Exited(Option<i64>),
    Failed(String),
    /// The adapter is gone; the session is over.
    Ended,
}

/// Sends commands to a running session.
#[derive(Clone)]
pub struct Handle {
    commands: mpsc::UnboundedSender<Command>,
    child: Arc<Mutex<Child>>,
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("dap::Handle")
    }
}

impl Handle {
    pub fn send(&self, command: Command) {
        let _ = self.commands.unbounded_send(command);
    }

    /// Stops the adapter (and with it the debugged program) right away.
    pub fn kill(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

enum Input {
    Command(Command),
    Message(Value),
    Closed,
}

/// What a pending request was for.
enum Pending {
    StackTrace,
    Scopes,
    /// Variables of one scope; the other scopes still to fetch.
    Variables(Vec<i64>),
    Evaluate(String),
    Other,
}

struct Session {
    launch: Launch,
    stdin: ChildStdin,
    seq: i64,
    pending: HashMap<i64, Pending>,
    thread: i64,
    frames: Vec<Frame>,
    frame_id: Option<i64>,
    /// Arguments and locals gathered across scopes for the current stop.
    collected: Vec<Variable>,
    /// Partial gdb console line (output arrives in pieces).
    line_buffer: String,
}

/// Starts the adapter and streams session events until the program ends.
pub fn run(launch: Launch) -> impl Stream<Item = Event> {
    iced::stream::channel(100, async move |mut output| {
        let (program, args) = match launch.command.split_first() {
            Some(split) => split,
            None => {
                let _ = output.send(Event::Failed("no debug adapter".into())).await;
                return;
            }
        };
        let mut command = platform::hidden_command(program);
        if launch.adapter == Adapter::Gdb {
            // gdb's built-in Python breaks when these point at another
            // Python (MSYSTEM is set by Git Bash / MSYS2 shells).
            command.env_remove("MSYSTEM").env_remove("PYTHONHOME").env_remove("PYTHONPATH");
        }
        // The debugged program inherits the adapter's environment.
        if let Some(cwd) = launch.arguments["cwd"].as_str() {
            crate::env::apply(&mut command, std::path::Path::new(cwd));
        }
        let spawned = command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(if std::env::var_os("DAP_TRACE").is_some() { Stdio::inherit() } else { Stdio::null() })
            .spawn();
        if std::env::var_os("DAP_TRACE").is_some() {
            eprintln!("ADAPTER {program} {args:?}");
        }
        let mut child = match spawned {
            Ok(child) => child,
            Err(e) => {
                let _ = output.send(Event::Failed(format!("Cannot start {program}: {e}"))).await;
                return;
            }
        };
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = output.send(Event::Failed("debug adapter has no pipes".into())).await;
            return;
        };

        let (message_tx, message_rx) = mpsc::unbounded();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Some(message) = read_message(&mut reader) {
                if message_tx.unbounded_send(Input::Message(message)).is_err() {
                    return;
                }
            }
            let _ = message_tx.unbounded_send(Input::Closed);
        });
        let (command_tx, command_rx) = mpsc::unbounded();
        let child = Arc::new(Mutex::new(child));
        let _ = output.send(Event::Started(Handle { commands: command_tx, child: child.clone() })).await;

        let mut session = Session {
            launch,
            stdin,
            seq: 0,
            pending: HashMap::new(),
            thread: 1,
            frames: Vec::new(),
            frame_id: None,
            collected: Vec::new(),
            line_buffer: String::new(),
        };
        let adapter_id = match session.launch.adapter {
            Adapter::Gdb => "gdb",
            Adapter::Debugpy => "debugpy",
            Adapter::NetCoreDbg => "coreclr",
        };
        session.request(
            "initialize",
            json!({
                "clientID": "rust-ide",
                "adapterID": adapter_id,
                "linesStartAt1": true,
                "columnsStartAt1": true,
                "pathFormat": "path",
            }),
            Pending::Other,
        );
        let arguments = session.launch.arguments.clone();
        session.request("launch", arguments, Pending::Other);

        let mut inputs = stream::select(command_rx.map(Input::Command), message_rx);
        while let Some(input) = inputs.next().await {
            let (events, done) = match input {
                Input::Command(command) => (session.command(command), false),
                Input::Message(message) => session.message(message),
                Input::Closed => (Vec::new(), true),
            };
            for event in events {
                let _ = output.send(event).await;
            }
            if done {
                break;
            }
        }
        if let Ok(mut child) = child.lock() {
            let _ = child.kill();
        }
        let _ = output.send(Event::Ended).await;
    })
}

impl Session {
    fn request(&mut self, command: &str, arguments: Value, pending: Pending) {
        self.seq += 1;
        self.pending.insert(self.seq, pending);
        let message = json!({ "seq": self.seq, "type": "request", "command": command, "arguments": arguments });
        let _ = write_message(&mut self.stdin, &message);
    }

    fn set_breakpoints(&mut self, path: &PathBuf, lines: &[usize]) {
        let breakpoints: Vec<Value> = lines.iter().map(|line| json!({ "line": line })).collect();
        let source = json!({ "path": path.to_string_lossy() });
        self.request("setBreakpoints", json!({ "source": source, "breakpoints": breakpoints }), Pending::Other);
    }

    /// Breakpoints for a file, plus the temporary one when it is in that file.
    fn lines_for(&self, path: &PathBuf) -> Vec<usize> {
        let mut lines = self.launch.breakpoints.get(path).cloned().unwrap_or_default();
        if let Some((temp_path, line)) = &self.launch.temporary
            && temp_path == path
            && !lines.contains(line)
        {
            lines.push(*line);
        }
        lines
    }

    fn command(&mut self, command: Command) -> Vec<Event> {
        let thread = json!({ "threadId": self.thread });
        match command {
            Command::Continue => self.request("continue", thread, Pending::Other),
            Command::Next => self.request("next", thread, Pending::Other),
            Command::StepIn => self.request("stepIn", thread, Pending::Other),
            Command::StepOut => self.request("stepOut", thread, Pending::Other),
            Command::SetBreakpoints { path, lines } => {
                self.launch.breakpoints.insert(path.clone(), lines);
                let lines = self.lines_for(&path);
                self.set_breakpoints(&path, &lines);
                return Vec::new();
            }
            Command::RunTo { path, line } => {
                self.launch.temporary = Some((path.clone(), line));
                let lines = self.lines_for(&path);
                self.set_breakpoints(&path, &lines);
                self.request("continue", thread, Pending::Other);
            }
            Command::Evaluate { expression } => {
                let mut arguments = json!({ "expression": expression, "context": "watch" });
                if let Some(frame) = self.frame_id {
                    arguments["frameId"] = json!(frame);
                }
                self.request("evaluate", arguments, Pending::Evaluate(expression));
                return Vec::new();
            }
            Command::Terminate => {
                self.request("disconnect", json!({ "terminateDebuggee": true }), Pending::Other);
                return Vec::new();
            }
        }
        vec![Event::Running]
    }

    /// Handles one adapter message; returns events and whether the session ended.
    fn message(&mut self, message: Value) -> (Vec<Event>, bool) {
        match message["type"].as_str() {
            Some("event") => self.event(&message),
            Some("response") => (self.response(&message), false),
            _ => (Vec::new(), false),
        }
    }

    fn event(&mut self, message: &Value) -> (Vec<Event>, bool) {
        let body = &message["body"];
        match message["event"].as_str().unwrap_or("") {
            "initialized" => {
                let files: Vec<PathBuf> = self
                    .launch
                    .breakpoints
                    .keys()
                    .cloned()
                    .chain(self.launch.temporary.iter().map(|(p, _)| p.clone()))
                    .collect();
                for path in files {
                    let lines = self.lines_for(&path);
                    self.set_breakpoints(&path, &lines);
                }
                if let Some(function) = self.launch.entry_function.clone() {
                    let breakpoints = json!([{ "name": function }]);
                    self.request("setFunctionBreakpoints", json!({ "breakpoints": breakpoints }), Pending::Other);
                }
                self.request("setExceptionBreakpoints", json!({ "filters": [] }), Pending::Other);
                self.request("configurationDone", json!({}), Pending::Other);
                (Vec::new(), false)
            }
            "stopped" => {
                self.thread = body["threadId"].as_i64().unwrap_or(self.thread);
                // A one-shot breakpoint is removed once anything stops.
                if let Some((path, _)) = self.launch.temporary.take() {
                    let lines = self.lines_for(&path);
                    self.set_breakpoints(&path, &lines);
                }
                if self.launch.entry_function.take().is_some() {
                    self.request("setFunctionBreakpoints", json!({ "breakpoints": [] }), Pending::Other);
                }
                let arguments = json!({ "threadId": self.thread, "levels": 20 });
                self.request("stackTrace", arguments, Pending::StackTrace);
                self.frames.clear();
                (Vec::new(), false)
            }
            "output" => {
                let category = body["category"].as_str().unwrap_or("console");
                let text = body["output"].as_str().unwrap_or("");
                if !matches!(category, "stdout" | "stderr") {
                    return (Vec::new(), false);
                }
                let text = if self.launch.adapter == Adapter::Gdb { self.filter_gdb(text) } else { text.to_string() };
                (if text.is_empty() { Vec::new() } else { vec![Event::Output(text)] }, false)
            }
            "exited" => (vec![Event::Exited(body["exitCode"].as_i64())], false),
            "terminated" => (Vec::new(), true),
            _ => (Vec::new(), false),
        }
    }

    fn filter_gdb(&mut self, text: &str) -> String {
        filter_gdb_output(&mut self.line_buffer, text)
    }

    fn response(&mut self, message: &Value) -> Vec<Event> {
        let seq = message["request_seq"].as_i64().unwrap_or(-1);
        let pending = self.pending.remove(&seq).unwrap_or(Pending::Other);
        let ok = message["success"].as_bool().unwrap_or(false);
        let body = &message["body"];
        match pending {
            Pending::StackTrace if ok => {
                self.frames = body["stackFrames"]
                    .as_array()
                    .map(|frames| {
                        frames
                            .iter()
                            .map(|f| Frame {
                                name: f["name"].as_str().unwrap_or("?").to_string(),
                                file: f["source"]["path"].as_str().map(PathBuf::from),
                                line: f["line"].as_u64().unwrap_or(0) as usize,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                self.frame_id = body["stackFrames"][0]["id"].as_i64();
                match self.frame_id {
                    Some(frame) => {
                        self.request("scopes", json!({ "frameId": frame }), Pending::Scopes);
                        Vec::new()
                    }
                    None => vec![self.stopped(Vec::new())],
                }
            }
            Pending::Scopes if ok => {
                // Arguments then locals; never registers or globals.
                let scopes = body["scopes"].as_array().cloned().unwrap_or_default();
                let wanted = |hint: &str, name: &str| {
                    scopes
                        .iter()
                        .filter(|s| s["presentationHint"] == hint || s["name"] == name)
                        .filter_map(|s| s["variablesReference"].as_i64())
                        .filter(|&r| r > 0)
                        .collect::<Vec<_>>()
                };
                let mut references = wanted("arguments", "Arguments");
                references.extend(wanted("locals", "Locals"));
                self.collected.clear();
                if references.is_empty() {
                    return vec![self.stopped(Vec::new())];
                }
                let first = references.remove(0);
                self.request("variables", json!({ "variablesReference": first }), Pending::Variables(references));
                Vec::new()
            }
            Pending::Variables(mut rest) => {
                let found: Vec<Variable> = body["variables"]
                    .as_array()
                    .map(|vars| {
                        vars.iter()
                            // debugpy groups like "special variables" have no value.
                            .filter(|v| !v["value"].as_str().unwrap_or("").is_empty())
                            .map(|v| Variable {
                                name: v["name"].as_str().unwrap_or("?").to_string(),
                                value: v["value"].as_str().unwrap_or("").to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                self.collected.extend(found);
                if rest.is_empty() {
                    let locals = std::mem::take(&mut self.collected);
                    return vec![self.stopped(locals)];
                }
                let next = rest.remove(0);
                self.request("variables", json!({ "variablesReference": next }), Pending::Variables(rest));
                Vec::new()
            }
            Pending::StackTrace | Pending::Scopes => vec![self.stopped(Vec::new())],
            Pending::Evaluate(expression) => {
                let value = if ok {
                    body["result"].as_str().unwrap_or("").to_string()
                } else {
                    format!("<{}>", message["message"].as_str().unwrap_or("not available"))
                };
                vec![Event::Evaluated { expression, value }]
            }
            Pending::Other => match message["command"].as_str() {
                Some("launch") if !ok => {
                    let reason = message["message"].as_str().unwrap_or("launch failed");
                    vec![Event::Failed(reason.to_string())]
                }
                _ => Vec::new(),
            },
        }
    }

    fn stopped(&self, locals: Vec<Variable>) -> Event {
        Event::Stopped { frames: self.frames.clone(), locals }
    }
}

/// Drops gdb's own console chatter, which it reports like program output.
/// Works on whole lines; an unfinished line waits in `buffer`.
fn filter_gdb_output(buffer: &mut String, text: &str) -> String {
    buffer.push_str(text);
    let Some(end) = buffer.rfind('\n') else { return String::new() };
    let complete: String = buffer.drain(..=end).collect();
    complete
        .split_inclusive('\n')
        .filter(|line| {
            let l = line.trim_start();
            !(l.starts_with("[Thread ")
                || l.starts_with("[New Thread")
                || l.starts_with("[Inferior ")
                || l.starts_with("No symbol table is loaded")
                || l.starts_with("No source file named")
                || l.starts_with("Breakpoint ")
                || l.starts_with("Temporary breakpoint")
                || l.starts_with("warning: "))
        })
        .collect()
}

/// The Python interpreter to run debugpy with: the project's `.venv` when it
/// has debugpy, else the system Python.
pub fn debugpy_python(root: &std::path::Path) -> Option<PathBuf> {
    let candidates = crate::lsp::venv_python(root)
        .into_iter()
        .chain(platform::find_program(platform::python()));
    candidates.into_iter().find(|python| {
        platform::hidden_command(python)
            .args(["-c", "import debugpy"])
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

/// netcoredbg, the C# debugger: next to the project (`.dotnet-tools`), in
/// the usual tool folders, or in a folder named by `NETCOREDBG`.
pub fn find_netcoredbg(root: &std::path::Path) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("NETCOREDBG") {
        dirs.push(PathBuf::from(dir));
    }
    for ancestor in root.ancestors() {
        dirs.push(ancestor.join(".dotnet-tools"));
        dirs.push(ancestor.join(".dotnet-tools").join("netcoredbg"));
    }
    dirs.extend(platform::search_dirs());
    for dir in platform::search_dirs() {
        dirs.push(dir.join("netcoredbg"));
    }
    platform::find_in(&dirs, "netcoredbg")
}

/// The built program of a C# project: `bin/<config>/<framework>/<name>.dll`,
/// the newest one (Debug builds only; the debugger needs symbols).
pub fn csharp_dll(project: &std::path::Path) -> Option<PathBuf> {
    let dir = project.parent()?;
    let name = project.file_stem()?.to_string_lossy().into_owned();
    let debug = dir.join("bin").join("Debug");
    std::fs::read_dir(debug)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path().join(format!("{name}.dll")))
        .filter(|p| p.is_file())
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
}

/// gdb with DAP support (14 or newer): the path and its version line.
pub fn find_gdb() -> Option<(PathBuf, String)> {
    let mut dirs = platform::search_dirs();
    // RHEL 9's gdb is too old; gcc-toolset provides a newer one.
    for toolset in ["gcc-toolset-15", "gcc-toolset-14", "gcc-toolset-13"] {
        dirs.insert(0, PathBuf::from(format!("/opt/rh/{toolset}/root/usr/bin")));
    }
    let gdb = platform::find_in(&dirs, "gdb")?;
    let output = platform::hidden_command(&gdb).arg("--version").output().ok()?;
    let version = String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or("").to_string();
    let major: u32 = version
        .split_whitespace()
        .last()
        .and_then(|v| v.split('.').next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    (major >= 14).then_some((gdb, version))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// gdb sessions in parallel disturb each other; the IDE only ever runs one.
    static GDB: Mutex<()> = Mutex::new(());

    fn probe_dir() -> PathBuf {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("dap-tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Runs a session, stepping on each stop, and returns what happened.
    fn drive(launch: Launch, steps: usize) -> (Vec<(usize, Vec<(String, String)>)>, String, Option<i64>) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            iced::futures::executor::block_on(async {
                let mut events = std::pin::pin!(run(launch));
                while let Some(event) = events.next().await {
                    if tx.send(event).is_err() {
                        break;
                    }
                }
            })
        });
        let deadline = Instant::now() + Duration::from_secs(90);
        let mut handle = None;
        let mut stops = Vec::new();
        let mut output = String::new();
        let mut exit = None;
        while let Ok(event) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            if std::env::var_os("DAP_TRACE").is_some() {
                eprintln!("EVENT {event:?}");
            }
            match event {
                Event::Started(h) => handle = Some(h),
                Event::Stopped { frames, locals, .. } => {
                    let line = frames.first().map_or(0, |f| f.line);
                    stops.push((line, locals.into_iter().map(|v| (v.name, v.value)).collect()));
                    let command = if stops.len() < steps { Command::Next } else { Command::Continue };
                    handle.as_ref().unwrap().send(command);
                }
                Event::Output(text) => output.push_str(&text),
                Event::Exited(code) => exit = code,
                Event::Failed(e) => panic!("{e}"),
                _ => {}
            }
        }
        (stops, output, exit)
    }

    #[test]
    #[ignore = "needs gdb 14+ with DAP; run with --ignored"]
    fn gdb_steps_through_rust() {
        let _one_at_a_time = GDB.lock().unwrap_or_else(|e| e.into_inner());
        let (gdb, _) = find_gdb().expect("gdb 14+");
        let dir = probe_dir();
        let source = dir.join("stepper.rs");
        std::fs::write(&source, "fn main() {\n    let x = 41;\n    let y = x + 1;\n    println!(\"answer {}\", y);\n}\n").unwrap();
        let exe = dir.join(format!("stepper{}", std::env::consts::EXE_SUFFIX));
        let status = platform::hidden_command(platform::find_program("rustc").unwrap())
            .arg("-g")
            .arg(&source)
            .arg("-o")
            .arg(&exe)
            .status()
            .unwrap();
        assert!(status.success());

        let launch = Launch {
            adapter: Adapter::Gdb,
            command: vec![gdb.to_string_lossy().into_owned(), "-q".into(), "-i".into(), "dap".into()],
            arguments: json!({ "program": exe, "cwd": dir }),
            entry_function: Some("stepper::main".into()),
            breakpoints: BTreeMap::new(),
            temporary: None,
        };
        let (stops, output, exit) = drive(launch, 3);
        assert_eq!(stops.iter().map(|s| s.0).collect::<Vec<_>>(), [2, 3, 4], "{stops:?}");
        assert!(stops[2].1.iter().any(|(n, v)| n == "y" && v == "42"), "{stops:?}");
        assert_eq!(output.trim(), "answer 42");
        assert_eq!(exit, Some(0));
    }

    #[test]
    #[ignore = "needs gdb 14+ with DAP; run with --ignored"]
    fn gdb_line_breakpoint_on_native_path() {
        let _one_at_a_time = GDB.lock().unwrap_or_else(|e| e.into_inner());
        let (gdb, _) = find_gdb().expect("gdb 14+");
        let dir = probe_dir();
        let source = dir.join("bp.rs");
        std::fs::write(&source, "fn main() {
    let a = 1;
    let b = a + 1;
    let c = b * 10;
    println!(\"{c}\");
}
").unwrap();
        let exe = dir.join(format!("bp{}", std::env::consts::EXE_SUFFIX));
        let status = platform::hidden_command(platform::find_program("rustc").unwrap())
            .arg("-g")
            .arg(&source)
            .arg("-o")
            .arg(&exe)
            .status()
            .unwrap();
        assert!(status.success());
        let launch = Launch {
            adapter: Adapter::Gdb,
            command: vec![gdb.to_string_lossy().into_owned(), "-q".into(), "-i".into(), "dap".into()],
            arguments: json!({ "program": exe, "cwd": dir }),
            entry_function: None,
            // The path exactly as the editor stores it (backslashes on Windows).
            breakpoints: BTreeMap::from([(source.clone(), vec![4])]),
            temporary: None,
        };
        let (stops, output, exit) = drive(launch, 1);
        assert_eq!(stops.iter().map(|s| s.0).collect::<Vec<_>>(), [4], "{stops:?}");
        assert!(stops[0].1.iter().any(|(n, v)| n == "b" && v == "2"), "{stops:?}");
        assert_eq!(output.trim(), "20");
        assert_eq!(exit, Some(0));
    }

    #[test]
    #[ignore = "needs debugpy; run with --ignored"]
    fn debugpy_stops_at_breakpoint() {
        let dir = probe_dir();
        let python = debugpy_python(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).expect("python with debugpy");
        let script = dir.join("stepper.py");
        std::fs::write(&script, "x = 41\ny = x + 1\nprint('answer', y)\nz = y * 2\n").unwrap();
        let launch = Launch {
            adapter: Adapter::Debugpy,
            command: vec![python.to_string_lossy().into_owned(), "-m".into(), "debugpy.adapter".into()],
            arguments: json!({
                "program": script,
                "cwd": dir,
                "console": "internalConsole",
                "justMyCode": true,
                "python": python,
            }),
            entry_function: None,
            breakpoints: BTreeMap::from([(script.clone(), vec![3])]),
            temporary: None,
        };
        let (stops, output, exit) = drive(launch, 2);
        assert_eq!(stops.iter().map(|s| s.0).collect::<Vec<_>>(), [3, 4], "{stops:?}");
        assert!(stops[0].1.iter().any(|(n, v)| n == "y" && v == "42"), "{stops:?}");
        assert!(output.contains("answer 42"), "{output:?}");
        assert_eq!(exit, Some(0));
    }

    #[test]
    #[ignore = "needs netcoredbg and the .NET SDK; run with --ignored"]
    fn netcoredbg_stops_at_breakpoint() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let debugger = find_netcoredbg(root).expect("netcoredbg (see .dotnet-tools)");
        let dir = probe_dir().join("csharp");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("demo.csproj"),
            "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <TargetFramework>net10.0</TargetFramework>\n  </PropertyGroup>\n</Project>\n",
        )
        .unwrap();
        let source = dir.join("Program.cs");
        std::fs::write(
            &source,
            "var x = 41;\nvar y = x + 1;\nSystem.Console.WriteLine($\"answer {y}\");\nvar z = y * 2;\n",
        )
        .unwrap();
        let build = platform::hidden_command(platform::find_program("dotnet").expect("dotnet"))
            .args(["build", "--nologo"])
            .arg(dir.join("demo.csproj"))
            .output()
            .unwrap();
        assert!(build.status.success(), "{}", String::from_utf8_lossy(&build.stdout));
        let dll = csharp_dll(&dir.join("demo.csproj")).expect("built dll");
        let launch = Launch {
            adapter: Adapter::NetCoreDbg,
            command: vec![debugger.to_string_lossy().into_owned(), "--interpreter=vscode".into()],
            arguments: json!({
                "program": platform::find_program("dotnet").unwrap(),
                "args": [dll],
                "cwd": dir,
                "stopAtEntry": false,
                "justMyCode": true,
            }),
            entry_function: None,
            breakpoints: BTreeMap::from([(source.clone(), vec![3])]),
            temporary: None,
        };
        let (stops, output, exit) = drive(launch, 2);
        assert_eq!(stops.iter().map(|s| s.0).collect::<Vec<_>>(), [3, 4], "{stops:?}");
        assert!(stops[0].1.iter().any(|(n, v)| n == "y" && v == "42"), "{stops:?}");
        assert!(output.contains("answer 42"), "{output:?}");
        assert_eq!(exit, Some(0));
    }

    #[test]
    fn gdb_chatter_is_filtered() {
        let mut buffer = String::new();
        let text = "answer 42\n[Thread 1.2 (id 3) exited with code 0]\nBreakpoint 1 (-function main) pending.\nlen";
        assert_eq!(filter_gdb_output(&mut buffer, text), "answer 42\n");
        assert_eq!(buffer, "len", "unfinished line waits");
        let mut other = String::new();
        assert_eq!(filter_gdb_output(&mut other, "No source file named C:\\x.rs.\n20\n"), "20\n");
        assert_eq!(filter_gdb_output(&mut buffer, " 3\n"), "len 3\n");
    }
}
