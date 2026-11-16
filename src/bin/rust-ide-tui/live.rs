//! Work that runs beside the editor: the program's Output window, the debugger and the
//! language servers. Their streams run on threads of the main loop, which hands each event
//! to [`App::background`].

use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use rust_ide::highlight::Lang;
use rust_ide::runner::RunTarget;
use rust_ide::{dap, lsp, output};

use crate::app::{App, Completion, Effect};

/// An event from a background stream.
pub enum Background {
    Lsp(lsp::Event),
    /// Output of the program run with this run id.
    Output(u64, output::Event),
    /// An event of the debug session with this id.
    Debug(u64, dap::Event),
    /// The tools found by the check at startup.
    Deps(Vec<rust_ide::deps::Dep>),
}

/// Turbo Pascal's user screen: the running program's output, and a line for its input.
#[derive(Default)]
pub struct OutputPane {
    pub screen: String,
    pub title: String,
    pub visible: bool,
    pub running: bool,
    pub run_id: u64,
    pub handle: Option<output::Handle>,
    pub input: String,
    /// Lines scrolled back from the end.
    pub scroll: usize,
}

/// A running debug session.
#[derive(Default)]
pub struct DebugSession {
    pub handle: Option<dap::Handle>,
    /// Where execution is stopped (file as shown in the editor, 0-based line).
    pub stopped_at: Option<(PathBuf, usize)>,
    pub frames: Vec<dap::Frame>,
    pub locals: Vec<dap::Variable>,
}

impl App {
    /// An event from a background thread.
    pub fn background(&mut self, event: Background) -> Effect {
        match event {
            Background::Lsp(event) => self.lsp_event(event),
            Background::Output(run_id, event) if run_id == self.output.run_id => self.output_event(event),
            Background::Debug(run_id, event) if run_id == self.debug_run_id => self.debug_event(event),
            Background::Deps(list) => self.deps_checked(list, true),
            _ => {}
        }
        Effect::None
    }

    // ----- Output window -----

    /// Starts a built program with its output in the Output window.
    pub fn start_run(&mut self, target: RunTarget) -> Effect {
        if let Some(handle) = self.output.handle.take() {
            handle.kill();
        }
        // For "python script.py" show the script name, otherwise the program.
        let shown = target.args.first().map(PathBuf::from).unwrap_or_else(|| target.program.clone());
        self.output.title = shown.file_name().unwrap_or_default().to_string_lossy().into_owned();
        self.output.screen.clear();
        self.output.input.clear();
        self.output.scroll = 0;
        self.output.running = true;
        self.output.visible = !target.gui;
        self.output.run_id += 1;
        self.status = if target.gui {
            "Running (GUI window) - Alt+F5 shows its output".into()
        } else {
            "Running - Alt+F5 toggles the output window".into()
        };
        Effect::StartRun { target, run_id: self.output.run_id }
    }

    fn output_event(&mut self, event: output::Event) {
        match event {
            output::Event::Started(handle) => self.output.handle = Some(handle),
            output::Event::Output(text) => output::append(&mut self.output.screen, &text),
            output::Event::Exited(code) => {
                self.output.running = false;
                self.output.handle = None;
                let code = code.map_or("stopped".to_string(), |c| format!("exit code {c}"));
                output::append(&mut self.output.screen, &format!("\n[Program finished, {code}]\n"));
                self.status = format!("Program finished, {code}");
            }
            output::Event::Failed(error) => {
                self.output.running = false;
                self.output.handle = None;
                output::append(&mut self.output.screen, &format!("{error}\n"));
                self.status = error;
            }
        }
    }

    /// Keys of the Output window: typing goes to the program's input line.
    pub fn output_key(&mut self, key: KeyEvent) -> Effect {
        let pane = &mut self.output;
        match key.code {
            KeyCode::Esc => pane.visible = false,
            KeyCode::Enter if pane.running => {
                let line = format!("{}\n", std::mem::take(&mut pane.input));
                if let Some(handle) = &pane.handle {
                    handle.send(&line);
                    // Piped programs do not echo what was typed.
                    output::append(&mut pane.screen, &line);
                }
                pane.scroll = 0;
            }
            KeyCode::Backspace => {
                pane.input.pop();
            }
            KeyCode::Char(c) if pane.running => pane.input.push(c),
            KeyCode::Up => pane.scroll += 1,
            KeyCode::Down => pane.scroll = pane.scroll.saturating_sub(1),
            KeyCode::PageUp => pane.scroll += 10,
            KeyCode::PageDown => pane.scroll = pane.scroll.saturating_sub(10),
            KeyCode::Home => pane.scroll = pane.screen.lines().count(),
            KeyCode::End => pane.scroll = 0,
            _ => {}
        }
        Effect::None
    }

    /// Ctrl+F2: stops the program (or the debug session).
    pub fn program_reset(&mut self) {
        if let Some(debug) = &self.debug {
            if let Some(handle) = &debug.handle {
                handle.send(dap::Command::Terminate);
                handle.kill();
            }
            self.status = "Program reset".into();
            return;
        }
        match &self.output.handle {
            Some(handle) => {
                handle.kill();
                self.status = "Program reset".into();
            }
            None => self.status = "No program is running".into(),
        }
    }

    /// Stops what is still running (when the IDE exits).
    pub fn stop_all(&mut self) {
        if let Some(handle) = self.output.handle.take() {
            handle.kill();
        }
        if let Some(handle) = self.debug.as_mut().and_then(|d| d.handle.take()) {
            handle.send(dap::Command::Terminate);
            handle.kill();
        }
    }

    // ----- Debugger -----

    pub fn debug_command(&self, command: dap::Command) {
        if let Some(handle) = self.debug.as_ref().and_then(|d| d.handle.as_ref()) {
            handle.send(command);
        }
    }

    pub fn toggle_breakpoint(&mut self) {
        let Some(path) = self.buffer().path.clone() else {
            self.status = "Save the file first to set breakpoints".into();
            return;
        };
        let line = self.buffer().row;
        let lines = self.breakpoints.entry(path.clone()).or_default();
        let added = lines.insert(line);
        if !added {
            lines.remove(&line);
        }
        let one_based: Vec<usize> = lines.iter().map(|l| l + 1).collect();
        if lines.is_empty() {
            self.breakpoints.remove(&path);
        }
        self.status = format!("Breakpoint {} on line {}", if added { "set" } else { "removed" }, line + 1);
        self.debug_command(dap::Command::SetBreakpoints { path, lines: one_based });
    }

    pub fn clear_breakpoints(&mut self) {
        let files: Vec<PathBuf> = std::mem::take(&mut self.breakpoints).into_keys().collect();
        for path in files {
            self.debug_command(dap::Command::SetBreakpoints { path, lines: Vec::new() });
        }
        self.status = "All breakpoints cleared".into();
    }

    pub fn add_watch(&mut self, expression: &str) {
        let expression = expression.trim().to_string();
        if !expression.is_empty() && !self.watches.iter().any(|(e, _)| *e == expression) {
            self.watches.push((expression.clone(), String::new()));
            self.debug_command(dap::Command::Evaluate { expression });
        }
    }

    pub fn start_debug(&mut self, target: RunTarget, start: dap::Start) -> Effect {
        let Some(path) = self.buffer().path.clone() else { return Effect::None };
        let launch = match dap::launch_for(&path, &target, &self.breakpoints, start) {
            Ok(launch) => launch,
            Err(problem) => {
                self.status = problem;
                return Effect::None;
            }
        };
        self.output.screen.clear();
        self.output.title = format!("{} (debugging)", path.file_name().unwrap_or_default().to_string_lossy());
        self.output.running = true;
        self.output.handle = None;
        self.debug = Some(DebugSession::default());
        self.debug_run_id += 1;
        self.status = "Debugging... (F7/F8 step, F5 continue, Ctrl+F2 stop)".into();
        Effect::StartDebug { launch, run_id: self.debug_run_id }
    }

    fn debug_event(&mut self, event: dap::Event) {
        match event {
            dap::Event::Started(handle) => {
                if let Some(debug) = &mut self.debug {
                    debug.handle = Some(handle);
                }
            }
            dap::Event::Stopped { frames, locals, .. } => {
                let top = frames.first().cloned();
                if let Some(debug) = &mut self.debug {
                    debug.frames = frames;
                    debug.locals = locals;
                    debug.stopped_at = None;
                }
                let Some(top) = top else {
                    self.status = "Stopped (no source here)".into();
                    return;
                };
                if let Some(file) = &top.file
                    && !self.is_current_file(file)
                {
                    self.open(file);
                    if !self.is_current_file(file) {
                        return;
                    }
                }
                let line = top.line.saturating_sub(1).min(self.buffer().lines.len() - 1);
                let indent = {
                    let text = &self.buffer().lines[line];
                    text[..text.len() - text.trim_start().len()].chars().count()
                };
                self.output.visible = false;
                self.mode = crate::app::Mode::Edit;
                self.buffer_mut().go_to(line, indent);
                let path = self.buffer().path.clone();
                if let (Some(debug), Some(path)) = (&mut self.debug, path) {
                    debug.stopped_at = Some((path, line));
                }
                self.status = format!("Stopped at line {} in {}", top.line, top.name);
                for (expression, _) in self.watches.clone() {
                    self.debug_command(dap::Command::Evaluate { expression });
                }
            }
            dap::Event::Running => {
                if let Some(debug) = &mut self.debug {
                    debug.stopped_at = None;
                }
                self.status = "Running... (Ctrl+F2 to stop)".into();
            }
            dap::Event::Output(text) => output::append(&mut self.output.screen, &text),
            dap::Event::Evaluated { expression, value } => {
                if let Some(watch) = self.watches.iter_mut().find(|(e, _)| *e == expression) {
                    watch.1 = value;
                }
            }
            dap::Event::Exited(code) => {
                let code = code.map_or("stopped".to_string(), |c| format!("exit code {c}"));
                output::append(&mut self.output.screen, &format!("\n[Program finished, {code}]\n"));
                self.status = format!("Program finished, {code} (Alt+F5 shows its output)");
            }
            dap::Event::Failed(error) => self.status = format!("Debugger: {error}"),
            dap::Event::Ended => {
                self.debug = None;
                self.output.running = false;
                if self.status.starts_with("Running") || self.status.starts_with("Debugging") {
                    self.status = "Debugging ended".into();
                }
            }
        }
    }

    /// The line execution is stopped on, when it is in the shown file.
    pub fn exec_line(&self) -> Option<usize> {
        let (file, line) = self.debug.as_ref()?.stopped_at.as_ref()?;
        (self.buffer().path.as_ref() == Some(file)).then_some(*line)
    }

    pub fn is_current_file(&self, file: &std::path::Path) -> bool {
        self.buffer().path.as_deref().is_some_and(|current| lsp::same_file(current, file))
    }

    // ----- Language servers -----

    fn lsp_send(&self, request: lsp::Request) {
        if let Some(lsp) = &self.lsp {
            let _ = lsp.unbounded_send(request);
        }
    }

    /// Keeps the language server on the file shown, and tells it about edits. Called after
    /// every key and event.
    pub fn sync(&mut self) {
        let version = (self.buffer().id, self.buffer().version);
        if version != self.synced {
            if version.0 == self.synced.0 {
                // The text changed: the last build's messages no longer fit it.
                self.messages.clear();
                self.message_index = None;
            } else {
                self.live.clear();
            }
            self.synced = version;
            if let Some(path) = self.lsp_doc.clone().filter(|_| version.0 == self.lsp_id) {
                self.lsp_send(lsp::Request::Change { path, text: self.buffer().text() });
            }
        }
        if self.lsp.is_none() {
            return;
        }
        let wanted = self.buffer().path.clone().filter(|p| Lang::from_path(Some(p)) != Lang::Plain);
        if wanted == self.lsp_doc {
            return;
        }
        if let Some(old) = self.lsp_doc.take() {
            self.lsp_send(lsp::Request::Close { path: old });
        }
        self.live.clear();
        if let Some(path) = wanted {
            self.lsp_send(lsp::Request::Open { path: path.clone(), text: self.buffer().text() });
            self.lsp_doc = Some(path);
            self.lsp_id = self.buffer().id;
        }
    }

    /// Tells the language server the file was saved.
    pub fn lsp_saved(&self) {
        if let Some(path) = &self.lsp_doc {
            self.lsp_send(lsp::Request::Save { path: path.clone() });
        }
    }

    fn lsp_event(&mut self, event: lsp::Event) {
        match event {
            lsp::Event::Ready(sender) => {
                self.lsp = Some(sender);
                self.sync();
            }
            lsp::Event::Status(status) => self.lsp_status = status,
            lsp::Event::Diagnostics { path, mut items } => {
                if self.is_current_file(&path) {
                    // Servers sometimes report the same problem twice (e.g. parser + checker).
                    items.dedup_by(|a, b| a.line == b.line && a.start == b.start && a.message == b.message);
                    self.live = items;
                }
            }
            lsp::Event::Completions { id, mut items } => {
                let Some(completion) = self.completion.as_ref().filter(|c| c.id == id && c.waiting) else { return };
                let start = completion.start;
                let b = self.buffer();
                let prefix = b.lines[b.row].get(start..b.col).unwrap_or("").to_string();
                // Words of the file and keywords after the server's own suggestions.
                if prefix.chars().count() >= 2 {
                    let known: std::collections::HashSet<String> = items.iter().map(|i| i.label.clone()).collect();
                    let extra = rust_ide::text::local_items(&b.text(), rust_ide::highlight::keywords(b.lang()), &prefix);
                    items.extend(extra.into_iter().filter(|i| !known.contains(&i.label)));
                }
                self.completion = Some(Completion { items, selected: 0, start, id, waiting: false });
                if self.visible_completions().is_empty() {
                    self.completion = None;
                    self.status = "No completions".into();
                }
            }
        }
    }

    /// Asks the language server what fits at the cursor (Ctrl+Space, or after `.` and `::`).
    /// Without a server, offers the words of the file and the keywords.
    pub fn request_completion(&mut self, trigger: Option<char>) {
        let Some(path) = self.lsp_doc.clone().filter(|_| self.lsp_id == self.buffer().id) else {
            self.complete(true);
            return;
        };
        let b = self.buffer();
        let before = &b.lines[b.row][..b.col];
        let start = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_').len();
        let (line, column) = (b.row, before.chars().count());
        self.next_completion_id += 1;
        let id = self.next_completion_id;
        self.completion = Some(Completion { items: Vec::new(), selected: 0, start, id, waiting: true });
        self.lsp_send(lsp::Request::Complete { id, path, line, column, trigger });
    }
}
