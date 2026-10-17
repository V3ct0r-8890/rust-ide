//! Minimal LSP client: one language server per (language, project root),
//! full-text document sync, diagnostics and completion.
//!
//! The servers run as child processes; a reader thread per server forwards
//! messages into one async loop that also receives requests from the UI.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt, stream};
use serde_json::{Value, json};

use crate::highlight::Lang;
use crate::platform;

#[derive(Debug, Clone)]
pub enum Event {
    Ready(mpsc::UnboundedSender<Request>),
    Status(String),
    Diagnostics {
        path: PathBuf,
        items: Vec<Diagnostic>,
    },
    Completions {
        id: u64,
        items: Vec<CompletionItem>,
    },
}

#[derive(Debug, Clone)]
pub enum Request {
    Open { path: PathBuf, text: String },
    Change { path: PathBuf, text: String },
    Save { path: PathBuf },
    Close { path: PathBuf },
    Complete { id: u64, path: PathBuf, line: usize, column: usize, trigger: Option<char> },
}

/// A diagnostic with 0-based lines and character (not UTF-16) columns.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub line: usize,
    pub start: usize,
    pub end_line: usize,
    pub end: usize,
    pub error: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub insert: String,
    pub detail: String,
    pub kind: &'static str,
    pub filter: String,
}

/// Starts the LSP client loop; the first event carries the request sender.
pub fn connect() -> impl Stream<Item = Event> {
    iced::stream::channel(100, async |mut output| {
        let (request_tx, request_rx) = mpsc::unbounded();
        let (message_tx, message_rx) = mpsc::unbounded();
        let _ = output.send(Event::Ready(request_tx)).await;

        let mut client = Client {
            servers: HashMap::new(),
            docs: HashMap::new(),
            message_tx,
        };
        let mut inputs = stream::select(
            request_rx.map(Input::Request),
            message_rx.map(|(key, message)| Input::Message(key, message)),
        );
        while let Some(input) = inputs.next().await {
            let events = match input {
                Input::Request(request) => client.request(request),
                Input::Message(key, message) => client.message(&key, message),
            };
            for event in events {
                let _ = output.send(event).await;
            }
        }
    })
}

type ServerKey = (Lang, PathBuf);

enum Input {
    Request(Request),
    Message(ServerKey, Value),
}

struct Server {
    name: String,
    stdin: ChildStdin,
    _child: Child,
    next_id: u64,
    initialized: bool,
    queued: Vec<Value>,
    utf32: bool,
    /// JSON-RPC id -> UI completion id.
    completions: HashMap<u64, u64>,
}

struct Doc {
    key: ServerKey,
    version: i64,
    text: String,
}

struct Client {
    servers: HashMap<ServerKey, Server>,
    docs: HashMap<PathBuf, Doc>,
    message_tx: mpsc::UnboundedSender<(ServerKey, Value)>,
}

const INITIALIZE_ID: u64 = 0;

impl Client {
    fn request(&mut self, request: Request) -> Vec<Event> {
        match request {
            Request::Open { path, text } => self.open(path, text),
            Request::Change { path, text } => {
                let Some(doc) = self.docs.get_mut(&path) else {
                    return self.open(path, text);
                };
                doc.version += 1;
                doc.text = text;
                let message = notification(
                    "textDocument/didChange",
                    json!({
                        "textDocument": { "uri": path_to_uri(&path), "version": doc.version },
                        "contentChanges": [{ "text": doc.text }],
                    }),
                );
                let key = doc.key.clone();
                self.send(&key, message);
                Vec::new()
            }
            Request::Save { path } => {
                if let Some(key) = self.docs.get(&path).map(|d| d.key.clone()) {
                    let params = json!({ "textDocument": { "uri": path_to_uri(&path) } });
                    self.send(&key, notification("textDocument/didSave", params));
                }
                Vec::new()
            }
            Request::Close { path } => {
                if let Some(doc) = self.docs.remove(&path) {
                    let params = json!({ "textDocument": { "uri": path_to_uri(&path) } });
                    self.send(&doc.key, notification("textDocument/didClose", params));
                }
                Vec::new()
            }
            Request::Complete { id, path, line, column, trigger } => {
                let Some(doc) = self.docs.get(&path) else {
                    return Vec::new();
                };
                let key = doc.key.clone();
                let Some(server) = self.servers.get_mut(&key) else {
                    return Vec::new();
                };
                let character = if server.utf32 {
                    column
                } else {
                    char_to_utf16(doc.text.lines().nth(line).unwrap_or(""), column)
                };
                let context = match trigger {
                    Some(c) => json!({ "triggerKind": 2, "triggerCharacter": c.to_string() }),
                    None => json!({ "triggerKind": 1 }),
                };
                server.next_id += 1;
                let rpc_id = server.next_id;
                server.completions.insert(rpc_id, id);
                let message = json!({
                    "jsonrpc": "2.0",
                    "id": rpc_id,
                    "method": "textDocument/completion",
                    "params": {
                        "textDocument": { "uri": path_to_uri(&path) },
                        "position": { "line": line, "character": character },
                        "context": context,
                    },
                });
                self.send(&key, message);
                Vec::new()
            }
        }
    }

    fn open(&mut self, path: PathBuf, text: String) -> Vec<Event> {
        let lang = Lang::from_path(Some(&path));
        if lang == Lang::Plain {
            return Vec::new();
        }
        let key = (lang, project_root(lang, &path));
        let mut events = Vec::new();
        if !self.servers.contains_key(&key) {
            match start_server(&key, &path, self.message_tx.clone()) {
                Ok(server) => {
                    events.push(Event::Status(format!("{}: starting", server.name)));
                    self.servers.insert(key.clone(), server);
                }
                Err(e) => {
                    events.push(Event::Status(e));
                    return events;
                }
            }
        }

        let language_id = if lang == Lang::Rust { "rust" } else { "python" };
        let message = notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": path_to_uri(&path),
                    "languageId": language_id,
                    "version": 1,
                    "text": text,
                }
            }),
        );
        self.send(&key, message);
        self.docs.insert(path, Doc { key, version: 1, text });
        events
    }

    /// Sends now, or after the server finished initializing.
    fn send(&mut self, key: &ServerKey, message: Value) {
        if let Some(server) = self.servers.get_mut(key) {
            if server.initialized {
                let _ = write_message(&mut server.stdin, &message);
            } else {
                server.queued.push(message);
            }
        }
    }

    fn message(&mut self, key: &ServerKey, message: Value) -> Vec<Event> {
        if message.get("$exit").is_some() {
            self.docs.retain(|_, doc| &doc.key != key);
            return match self.servers.remove(key) {
                Some(server) => vec![Event::Status(format!("{} stopped", server.name))],
                None => Vec::new(),
            };
        }
        let Some(server) = self.servers.get_mut(key) else {
            return Vec::new();
        };
        let method = message["method"].as_str();
        let id = message.get("id").cloned();

        match (method, id) {
            // A request from the server: always answer so it never waits on us.
            (Some(method), Some(id)) => {
                let result = match method {
                    "workspace/configuration" => configuration(key, &message["params"]),
                    _ => Value::Null,
                };
                let reply = json!({ "jsonrpc": "2.0", "id": id, "result": result });
                let _ = write_message(&mut server.stdin, &reply);
                Vec::new()
            }
            // A response to one of our requests.
            (None, Some(id)) => {
                let id = id.as_u64().unwrap_or(u64::MAX);
                if id == INITIALIZE_ID {
                    server.initialized = true;
                    server.utf32 =
                        message["result"]["capabilities"]["positionEncoding"] == "utf-32";
                    let _ = write_message(&mut server.stdin, &notification("initialized", json!({})));
                    for queued in std::mem::take(&mut server.queued) {
                        let _ = write_message(&mut server.stdin, &queued);
                    }
                    return vec![Event::Status(format!("{}: ready", server.name))];
                }
                match server.completions.remove(&id) {
                    Some(ui_id) => vec![Event::Completions {
                        id: ui_id,
                        items: parse_completions(&message["result"]),
                    }],
                    None => Vec::new(),
                }
            }
            (Some("textDocument/publishDiagnostics"), None) => {
                let params = &message["params"];
                let Some(path) = params["uri"].as_str().and_then(uri_to_path) else {
                    return Vec::new();
                };
                let utf32 = server.utf32;
                let text = self
                    .docs
                    .iter()
                    .find(|(p, _)| same_file(p, &path))
                    .map(|(_, doc)| doc.text.as_str())
                    .unwrap_or("");
                let items = params["diagnostics"]
                    .as_array()
                    .map(|list| list.iter().filter_map(|d| parse_diagnostic(d, text, utf32)).collect())
                    .unwrap_or_default();
                vec![Event::Diagnostics { path, items }]
            }
            (Some("$/progress"), None) => {
                let value = &message["params"]["value"];
                let name = &server.name;
                match value["kind"].as_str() {
                    Some("end") => vec![Event::Status(format!("{name}: ready"))],
                    Some(_) => {
                        let title = value["title"].as_str().unwrap_or("");
                        let detail = value["message"].as_str().unwrap_or("");
                        vec![Event::Status(format!("{name}: {title} {detail}").trim_end().to_string())]
                    }
                    None => Vec::new(),
                }
            }
            (Some("window/showMessage"), None) if message["params"]["type"] == 1 => {
                let text = message["params"]["message"].as_str().unwrap_or("");
                vec![Event::Status(format!("{}: {text}", server.name))]
            }
            _ => Vec::new(),
        }
    }
}

/// Answers `workspace/configuration`: standard type checking for pyright,
/// and the project's `.venv` interpreter when there is one.
fn configuration(key: &ServerKey, params: &Value) -> Value {
    let items = params["items"].as_array().cloned().unwrap_or_default();
    let venv_python = venv_dir(&key.1).map(|venv| venv.join(PYTHON_EXE));
    // "standard": real errors, without the strict mode's warnings about
    // missing type annotations on every untyped function.
    let analysis = json!({ "typeCheckingMode": "standard" });
    Value::Array(
        items
            .iter()
            // basedpyright asks for "basedpyright" (analysis nested inside),
            // pyright for "python" and "python.analysis": answer every form.
            .map(|item| match item["section"].as_str() {
                Some(s) if s.ends_with(".analysis") => analysis.clone(),
                Some("basedpyright") => json!({ "analysis": analysis }),
                Some("python") => {
                    let mut python = json!({ "analysis": analysis });
                    if let Some(path) = &venv_python {
                        python["pythonPath"] = json!(path);
                    }
                    python
                }
                _ => Value::Null,
            })
            .collect(),
    )
}

fn start_server(
    key: &ServerKey,
    file: &Path,
    message_tx: mpsc::UnboundedSender<(ServerKey, Value)>,
) -> Result<Server, String> {
    let (lang, root) = key;
    let (program, args, name) = match lang {
        Lang::Rust => (
            find_program(root, &["rust-analyzer"])
                .ok_or("rust-analyzer not installed (see Options > Dependencies)")?,
            Vec::new(),
            "rust-analyzer",
        ),
        _ => {
            let program = find_program(root, PYTHON_SERVERS)
                .ok_or("Python LSP not installed (see Options > Dependencies)")?;
            (program, vec!["--stdio"], "pyright")
        }
    };

    let mut command = platform::hidden_command(&program);
    command
        .args(&args)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot start {}: {e}", program.display()))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;

    let reader_key = key.clone();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        while let Some(message) = read_message(&mut reader) {
            if message_tx.unbounded_send((reader_key.clone(), message)).is_err() {
                return;
            }
        }
        let _ = message_tx.unbounded_send((reader_key, json!({ "$exit": true })));
    });

    // rust-analyzer needs to be told about .rs files outside a Cargo project.
    let standalone = *lang == Lang::Rust && !root.join("Cargo.toml").is_file();
    let options = if standalone {
        json!({ "detachedFiles": [file] })
    } else {
        json!({})
    };
    let root_uri = path_to_uri(root);
    let initialize = json!({
        "jsonrpc": "2.0",
        "id": INITIALIZE_ID,
        "method": "initialize",
        "params": {
            "processId": std::process::id(),
            "rootUri": root_uri,
            "workspaceFolders": [{ "uri": root_uri, "name": root.file_name().map(|n| n.to_string_lossy()).unwrap_or_default() }],
            "initializationOptions": options,
            "capabilities": {
                "general": { "positionEncodings": ["utf-32", "utf-16"] },
                "window": { "workDoneProgress": true },
                "workspace": { "configuration": true, "workspaceFolders": true },
                "textDocument": {
                    "synchronization": { "didSave": true },
                    "publishDiagnostics": {},
                    "completion": { "completionItem": { "snippetSupport": false } },
                },
            },
        },
    });
    write_message(&mut stdin, &initialize).map_err(|e| format!("{name}: {e}"))?;

    Ok(Server {
        name: name.to_string(),
        stdin,
        _child: child,
        next_id: INITIALIZE_ID,
        initialized: false,
        queued: Vec::new(),
        utf32: false,
        completions: HashMap::new(),
    })
}

#[cfg(windows)]
const PYTHON_EXE: &str = "Scripts/python.exe";
#[cfg(not(windows))]
const PYTHON_EXE: &str = "bin/python";

/// Finds the first of `names` in the project's `.venv`, on PATH, in the
/// usual per-user tool directories, or in Python's user scripts directory.
pub fn find_program(root: &Path, names: &[&str]) -> Option<PathBuf> {
    let bin = if cfg!(windows) { "Scripts" } else { "bin" };
    let mut dirs: Vec<PathBuf> = venv_dir(root).map(|v| v.join(bin)).into_iter().collect();
    dirs.extend(platform::search_dirs());
    dirs.extend(python_user_scripts());
    names.iter().find_map(|name| platform::find_in(&dirs, name))
}

/// The Python language servers this IDE can use, in order of preference.
pub const PYTHON_SERVERS: &[&str] = &["basedpyright-langserver", "pyright-langserver"];

fn venv_dir(root: &Path) -> Option<PathBuf> {
    root.ancestors()
        .flat_map(|dir| [dir.join(".venv"), dir.join("venv")])
        .find(|venv| venv.join(PYTHON_EXE).is_file())
}

/// The Python interpreter of the project's virtual environment, if any.
pub fn venv_python(root: &Path) -> Option<PathBuf> {
    venv_dir(root).map(|venv| venv.join(PYTHON_EXE))
}

/// Python's per-user scripts directory (asked once; spawning Python is slow).
fn python_user_scripts() -> Option<PathBuf> {
    static DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let script = "import os, sysconfig; print(sysconfig.get_path('scripts', f'{os.name}_user'))";
        let output =
            platform::hidden_command(platform::python()).args(["-c", script]).output().ok()?;
        let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (!dir.is_empty()).then(|| PathBuf::from(dir))
    })
    .clone()
}

/// The directory a language server should treat as the workspace.
fn project_root(lang: Lang, file: &Path) -> PathBuf {
    let dir = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    if lang == Lang::Rust {
        // Same rule as the build: only files in a package's source folders.
        return crate::runner::find_cargo_manifest(file)
            .and_then(|manifest| manifest.parent().map(Path::to_path_buf))
            .unwrap_or(dir);
    }
    let markers: &[&str] = match lang {
        Lang::Rust => &["Cargo.toml"],
        _ => &["pyproject.toml", "setup.py", ".venv"],
    };
    dir.ancestors()
        .find(|d| markers.iter().any(|m| d.join(m).exists()))
        .map(Path::to_path_buf)
        .unwrap_or(dir)
}

fn notification(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

pub fn write_message(stdin: &mut impl Write, message: &Value) -> std::io::Result<()> {
    let body = message.to_string();
    write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len())?;
    stdin.flush()
}

pub fn read_message(reader: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None;
        }
        let header = header.trim();
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn parse_diagnostic(diagnostic: &Value, text: &str, utf32: bool) -> Option<Diagnostic> {
    let error = match diagnostic["severity"].as_u64() {
        Some(1) | None => true,
        Some(2) => false,
        _ => return None, // information and hints
    };
    let range = &diagnostic["range"];
    let position = |p: &Value| -> (usize, usize) {
        let line = p["line"].as_u64().unwrap_or(0) as usize;
        let character = p["character"].as_u64().unwrap_or(0) as usize;
        let column = if utf32 {
            character
        } else {
            utf16_to_char(text.lines().nth(line).unwrap_or(""), character)
        };
        (line, column)
    };
    let (line, start) = position(&range["start"]);
    let (end_line, end) = position(&range["end"]);
    let message = diagnostic["message"].as_str()?.lines().next().unwrap_or("").to_string();
    Some(Diagnostic { line, start, end_line, end, error, message })
}

fn parse_completions(result: &Value) -> Vec<CompletionItem> {
    let list = result
        .as_array()
        .or_else(|| result["items"].as_array())
        .cloned()
        .unwrap_or_default();
    let mut items: Vec<(String, CompletionItem)> = list
        .iter()
        .filter_map(|item| {
            let label = item["label"].as_str()?.to_string();
            let insert = item["textEdit"]["newText"]
                .as_str()
                .or_else(|| item["insertText"].as_str())
                .unwrap_or(&label)
                .to_string();
            let sort = item["sortText"].as_str().unwrap_or(&label).to_string();
            let filter = item["filterText"].as_str().unwrap_or(&label).to_string();
            Some((
                sort,
                CompletionItem {
                    detail: item["detail"].as_str().unwrap_or("").to_string(),
                    kind: completion_kind(item["kind"].as_u64().unwrap_or(0)),
                    label,
                    insert,
                    filter,
                },
            ))
        })
        .collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    items.into_iter().map(|(_, item)| item).take(300).collect()
}

fn completion_kind(kind: u64) -> &'static str {
    match kind {
        2 => "method",
        3 => "fn",
        4 => "new",
        5 => "field",
        6 => "var",
        7 => "class",
        8 => "trait",
        9 => "module",
        10 => "prop",
        13 => "enum",
        14 => "keyword",
        15 => "snippet",
        20 => "variant",
        21 => "const",
        22 => "struct",
        25 => "type",
        _ => "",
    }
}

fn char_to_utf16(line: &str, column: usize) -> usize {
    line.chars().take(column).map(char::len_utf16).sum()
}

fn utf16_to_char(line: &str, units: usize) -> usize {
    let mut seen = 0;
    for (i, c) in line.chars().enumerate() {
        if seen >= units {
            return i;
        }
        seen += c.len_utf16();
    }
    line.chars().count()
}

pub fn path_to_uri(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = path.strip_prefix("//?/").unwrap_or(&path);
    let mut uri = String::from(if path.starts_with('/') { "file://" } else { "file:///" });
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                uri.push(byte as char)
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let mut bytes = Vec::with_capacity(rest.len());
    let mut iter = rest.bytes();
    while let Some(b) = iter.next() {
        if b == b'%' {
            let hex = [iter.next()?, iter.next()?];
            bytes.push(u8::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?);
        } else {
            bytes.push(b);
        }
    }
    let decoded = String::from_utf8(bytes).ok()?;
    // "/C:/dir/file" on Windows -> "C:/dir/file"
    let trimmed = match decoded.as_bytes() {
        [b'/', _, b':', ..] => &decoded[1..],
        _ => &decoded,
    };
    Some(PathBuf::from(trimmed))
}

pub fn same_file(a: &Path, b: &Path) -> bool {
    a == b || std::fs::canonicalize(a).ok() == std::fs::canonicalize(b).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_round_trip() {
        let path = Path::new(r"G:\My Projects\ไทย\main.rs");
        let uri = path_to_uri(path);
        assert!(uri.starts_with("file:///G:/My%20Projects/"));
        assert_eq!(uri_to_path(&uri).unwrap(), PathBuf::from("G:/My Projects/ไทย/main.rs"));
        // rust-analyzer style: lowercase drive and encoded colon
        assert_eq!(uri_to_path("file:///g%3A/a/b.rs").unwrap(), PathBuf::from("g:/a/b.rs"));
    }

    #[test]
    fn utf16_columns() {
        let line = "let s = \"😀\"; x";
        assert_eq!(char_to_utf16(line, 10), 11);
        assert_eq!(utf16_to_char(line, 11), 10);
    }

    /// Drives the client against a real server: waits for an error
    /// diagnostic, then asks for completions after `v.` on `line`.
    fn live_server(file: &Path, source: &str, line: usize, column: usize, expected: &str) {
        use std::sync::mpsc as std_mpsc;
        use std::time::{Duration, Instant};

        std::fs::write(file, source).unwrap();
        let (tx, rx) = std_mpsc::channel();
        std::thread::spawn(move || {
            iced::futures::executor::block_on(async {
                let mut events = std::pin::pin!(connect());
                while let Some(event) = events.next().await {
                    if tx.send(event).is_err() {
                        break;
                    }
                }
            })
        });
        let deadline = Instant::now() + Duration::from_secs(120);
        let next = || rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).ok();

        let Some(Event::Ready(requests)) = next() else { panic!("no Ready event") };
        let path = file.to_path_buf();
        requests.unbounded_send(Request::Open { path: path.clone(), text: source.into() }).unwrap();

        let mut statuses = Vec::new();
        let diagnostics = loop {
            match next() {
                Some(Event::Diagnostics { items, .. }) if items.iter().any(|d| d.error) => break items,
                Some(Event::Status(s)) => statuses.push(s),
                Some(_) => {}
                None => panic!("no error diagnostic; statuses: {statuses:?}"),
            }
        };
        assert!(diagnostics.iter().any(|d| d.error), "{diagnostics:?}");

        requests
            .unbounded_send(Request::Complete { id: 7, path, line, column, trigger: Some('.') })
            .unwrap();
        let items = loop {
            match next() {
                Some(Event::Completions { id: 7, items }) => break items,
                Some(_) => {}
                None => panic!("no completions"),
            }
        };
        assert!(items.iter().any(|i| i.label.starts_with(expected)), "{:?}", items.iter().map(|i| &i.label).collect::<Vec<_>>());
    }

    fn test_dir(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(name);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        dir
    }

    #[test]
    #[ignore = "needs rust-analyzer; run with --ignored"]
    fn rust_analyzer_live() {
        let dir = test_dir("lsp-rust");
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n").unwrap();
        let source = "fn main() {\n    let v: Vec<i32> = Vec::new();\n    let n: i32 = \"oops\";\n    v.\n}\n";
        live_server(&dir.join("src").join("main.rs"), source, 3, 6, "push");
    }

    #[test]
    #[ignore = "needs basedpyright in the project .venv; run with --ignored"]
    fn pyright_live() {
        let dir = test_dir("lsp-python");
        let source = "v: list[int] = []\nn: int = \"oops\"\nv.\n";
        live_server(&dir.join("demo.py"), source, 2, 2, "append");
    }

    #[test]
    fn frames_messages() {
        let input = b"Content-Length: 17\r\n\r\n{\"jsonrpc\":\"2.0\"}";
        let message = read_message(&mut &input[..]).unwrap();
        assert_eq!(message["jsonrpc"], "2.0");
    }
}
