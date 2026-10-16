//! rust-ide: a Turbo Pascal–style IDE for Rust and Python (iced GUI front end).

mod deps;
mod highlight;
mod lsp;
mod output;
mod platform;
mod runner;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::futures::channel::mpsc;
use iced::keyboard::{self, Key, Modifiers, key};
use iced::widget::text_editor::{Action, Binding, Cursor, Edit, KeyPress, Position};
use iced::widget::{
    Space, button, checkbox, column, container, mouse_area, opaque, row, scrollable, stack, text,
    text_editor, text_input,
};
use iced::{Background, Color, Element, Font, Length, Padding, Subscription, Task, Theme};

use highlight::{Lang, Mark};
use platform::Os;
use runner::{BuildResult, Diagnostic, RunTarget};

const TURBO_BLUE: Color = Color::from_rgb(0.0, 0.0, 0.66);
const TURBO_CYAN: Color = Color::from_rgb(0.0, 0.66, 0.66);
const TURBO_GRAY: Color = Color::from_rgb(0.75, 0.75, 0.75);
const TURBO_DARK_GRAY: Color = Color::from_rgb(0.33, 0.33, 0.33);
const TURBO_GREEN: Color = Color::from_rgb(0.0, 0.66, 0.0);
const TURBO_YELLOW: Color = Color::from_rgb(1.0, 1.0, 0.33);
const TURBO_RED: Color = Color::from_rgb(0.66, 0.0, 0.0);
const SCREEN_TEXT: Color = Color::from_rgb(0.75, 0.75, 0.75);

const MENU_TITLES: [&str; 9] = [
    "File", "Edit", "Search", "Run", "Compile", "Debug", "Options", "Window", "Help",
];
const MENU_TITLE_WIDTH: f32 = 84.0;
const MENU_BAR_HEIGHT: f32 = 26.0;
const STATUS_BAR_HEIGHT: f32 = 24.0;
const MESSAGES_HEIGHT: f32 = 140.0;
const COMPLETION_ROWS: usize = 10;
const OUTPUT_INPUT: &str = "output-input";

fn main() -> iced::Result {
    if let Err(message) = platform::gui_preflight() {
        eprintln!("{message}");
        std::process::exit(1);
    }
    // Opt-in software rendering, for machines with broken GPU drivers.
    let software = std::env::args().any(|a| a == "--software")
        || platform::read_setting("renderer").as_deref() == Some("software");
    if software && std::env::var_os("ICED_BACKEND").is_none() {
        // SAFETY: called before any other thread is started.
        unsafe { std::env::set_var("ICED_BACKEND", "tiny-skia") };
    }

    iced::application(Ide::new, Ide::update, Ide::view)
        .title(Ide::title)
        .theme(|_: &Ide| Theme::Dark)
        .subscription(Ide::subscription)
        .default_font(Font::MONOSPACE)
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Menu {
    File,
    Run,
    Compile,
    Options,
    Help,
}

/// What to do once the file is saved (Save As may be pending).
#[derive(Debug, Clone, Copy, PartialEq)]
enum AfterSave {
    Nothing,
    Compile,
    Run,
    RunExternal,
}

/// Commands that throw away unsaved edits and therefore need a second press.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Discard {
    New,
    Open,
    Exit,
}

/// Keys that drive the completion popup while it is open.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Nav {
    Up,
    Down,
    Accept,
    Cancel,
}

struct Completion {
    id: u64,
    /// Where the word being completed starts.
    anchor: Position,
    items: Vec<lsp::CompletionItem>,
    selected: usize,
    waiting: bool,
}

/// Turbo Pascal's "User screen": the running program's input and output.
#[derive(Default)]
struct OutputPane {
    screen: String,
    title: String,
    visible: bool,
    running: bool,
    run_id: u64,
    handle: Option<output::Handle>,
    input: String,
}

struct DepsDialog {
    list: Vec<deps::Dep>,
    checking: bool,
    installing: Option<&'static str>,
    message: String,
}

#[derive(Default)]
struct Ide {
    content: text_editor::Content,
    path: Option<PathBuf>,
    modified: bool,
    menu: Option<Menu>,
    confirm: Option<Discard>,
    building: bool,
    /// Results of the last F9/F5 build.
    diagnostics: Vec<Diagnostic>,
    status: String,
    lsp: Option<mpsc::UnboundedSender<lsp::Request>>,
    /// The file currently open in the language server.
    lsp_doc: Option<PathBuf>,
    lsp_status: String,
    /// Live diagnostics for the current file from the language server.
    live: Vec<lsp::Diagnostic>,
    marks: Arc<Vec<Mark>>,
    completion: Option<Completion>,
    next_completion_id: u64,
    output: OutputPane,
    os: Option<Os>,
    deps: Option<DepsDialog>,
    check_at_startup: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    ToggleMenu(Menu),
    CloseMenu,
    New,
    Open,
    Opened(Option<PathBuf>),
    Save,
    SaveAs,
    SaveAsPicked(Option<PathBuf>, AfterSave),
    Exit,
    Compile,
    Run,
    BuildFinished(BuildResult, AfterSave),
    JumpTo(usize),
    Lsp(lsp::Event),
    TriggerCompletion,
    CompletionNav(Nav),
    CompletionPick(usize),
    Escape,
    RunExternal,
    Output(u64, output::Event),
    OutputInput(String),
    OutputSubmit,
    OutputToggle,
    OutputStop,
    ShowDeps,
    DepsChecked(Vec<deps::Dep>, bool),
    DepInstall(usize),
    DepInstalled(&'static str, Result<String, String>),
    ToggleStartupCheck(bool),
    CloseDeps,
    CopyText(String),
    OpenUrl(&'static str),
}

impl Ide {
    /// Starts empty, or with the file given on the command line, and checks
    /// the external tools in the background.
    fn new() -> (Self, Task<Message>) {
        let mut ide = Self::default();
        let os = Os::detect();
        ide.os = Some(os);
        ide.check_at_startup =
            platform::read_setting("check_dependencies").as_deref() != Some("false");
        let file = std::env::args_os().skip(1).find(|a| !a.to_string_lossy().starts_with("--"));
        if let Some(path) = file.map(PathBuf::from) {
            if path.exists() {
                ide.load(&path);
            } else {
                ide.status = format!("New file {}", path.display());
                ide.path = Some(path);
            }
        }
        let task = if ide.check_at_startup {
            Task::perform(platform::blocking(move || deps::check(os)), |list| Message::DepsChecked(list, true))
        } else {
            Task::none()
        };
        (ide, task)
    }

    fn title(&self) -> String {
        let name = self
            .path
            .as_deref()
            .and_then(Path::file_name)
            .map_or("NONAME.RS".into(), |n| n.to_string_lossy().into_owned());
        let dirty = if self.modified { " *" } else { "" };
        format!("{name}{dirty} - Rust IDE")
    }

    fn subscription(&self) -> Subscription<Message> {
        // Shortcuts while the editor is not focused (the editor handles its own).
        let keys = keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                ..
            } => shortcut(&key, physical_key, modifiers),
            _ => None,
        });
        // Esc must work even when a text field has focus and captures it.
        let escape = iced::event::listen_with(|event, _status, _window| match event {
            iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(key::Named::Escape),
                ..
            }) => Some(Message::Escape),
            _ => None,
        });
        Subscription::batch([keys, escape, Subscription::run(lsp::connect).map(Message::Lsp)])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        if !matches!(message, Message::Edit(_) | Message::Lsp(_)) {
            self.menu = None;
        }
        // Background LSP events must not cancel a pending "press again to discard".
        let confirm = if matches!(message, Message::Lsp(_)) { None } else { self.confirm.take() };

        match message {
            Message::Edit(action) => self.edit(action),
            Message::ToggleMenu(menu) => {
                self.menu = (self.menu != Some(menu)).then_some(menu);
                Task::none()
            }
            Message::CloseMenu => Task::none(),
            Message::New => {
                if self.needs_confirm(confirm, Discard::New) {
                    return Task::none();
                }
                self.content = text_editor::Content::new();
                self.path = None;
                self.modified = false;
                self.diagnostics.clear();
                self.set_live(Vec::new());
                self.completion = None;
                self.status = "New file".into();
                self.lsp_sync_doc();
                Task::none()
            }
            Message::Open => {
                if self.needs_confirm(confirm, Discard::Open) {
                    return Task::none();
                }
                Task::perform(
                    rfd::AsyncFileDialog::new()
                        .set_title("Open")
                        .add_filter("Rust / Python", &["rs", "py", "pyw"])
                        .add_filter("All files", &["*"])
                        .pick_file(),
                    |file| Message::Opened(file.map(|f| f.path().to_path_buf())),
                )
            }
            Message::Opened(Some(path)) => {
                self.load(&path);
                Task::none()
            }
            Message::Opened(None) => Task::none(),
            Message::Save => self.save(AfterSave::Nothing),
            Message::SaveAs => save_as_dialog(AfterSave::Nothing),
            Message::SaveAsPicked(Some(path), after) => {
                self.path = Some(path);
                self.modified = true; // force a write to the new location
                let task = self.save(after);
                self.lsp_sync_doc();
                task
            }
            Message::SaveAsPicked(None, _) => {
                self.status = "Save cancelled".into();
                Task::none()
            }
            Message::Exit => {
                if self.needs_confirm(confirm, Discard::Exit) {
                    return Task::none();
                }
                iced::exit()
            }
            Message::Compile => self.save(AfterSave::Compile),
            Message::Run => self.save(AfterSave::Run),
            Message::RunExternal => self.save(AfterSave::RunExternal),
            Message::BuildFinished(result, after) => {
                self.building = false;
                self.finish_build(result, after)
            }
            Message::JumpTo(index) => {
                if let Some(diagnostic) = self.panel_items().get(index).cloned() {
                    self.jump_to(&diagnostic);
                }
                Task::none()
            }
            Message::Lsp(event) => {
                self.lsp_event(event);
                Task::none()
            }
            Message::TriggerCompletion => {
                self.request_completion(None);
                Task::none()
            }
            Message::CompletionNav(nav) => {
                self.completion_nav(nav);
                Task::none()
            }
            Message::CompletionPick(index) => {
                self.accept_completion(index);
                Task::none()
            }
            Message::Escape => {
                if self.deps.is_some() {
                    self.deps = None;
                } else if self.output.visible {
                    self.output.visible = false;
                }
                Task::none()
            }
            Message::Output(run_id, event) => {
                if run_id == self.output.run_id {
                    self.output_event(event);
                }
                Task::none()
            }
            Message::OutputInput(input) => {
                self.output.input = input;
                Task::none()
            }
            Message::OutputSubmit => {
                let line = format!("{}\n", std::mem::take(&mut self.output.input));
                if let Some(handle) = &self.output.handle {
                    handle.send(&line);
                    // Piped programs do not echo what was typed.
                    output::append(&mut self.output.screen, &line);
                }
                Task::none()
            }
            Message::OutputToggle => {
                self.output.visible = !self.output.visible;
                if self.output.visible {
                    iced::widget::operation::focus(OUTPUT_INPUT)
                } else {
                    Task::none()
                }
            }
            Message::OutputStop => {
                match &self.output.handle {
                    Some(handle) => {
                        handle.kill();
                        self.status = "Program reset".into();
                    }
                    None => self.status = "No program is running".into(),
                }
                Task::none()
            }
            Message::ShowDeps => self.check_deps(false),
            Message::DepsChecked(list, startup) => {
                let missing = list.iter().any(|d| d.missing() && d.need != deps::Need::Optional);
                if startup && !missing {
                    return Task::none();
                }
                let keep = self.deps.as_ref().filter(|d| d.checking).map(|d| d.message.clone());
                let message = keep.unwrap_or_else(|| {
                    if missing {
                        "Some tools are missing. Install them here, or open the install guide."
                    } else {
                        "Everything needed is installed."
                    }
                    .into()
                });
                self.deps = Some(DepsDialog { list, checking: false, installing: None, message });
                Task::none()
            }
            Message::DepInstall(index) => {
                let Some(dialog) = &mut self.deps else { return Task::none() };
                let Some(dep) = dialog.list.get(index) else { return Task::none() };
                let Some(command) = dep.auto.clone() else { return Task::none() };
                let name = dep.name;
                dialog.installing = Some(name);
                dialog.message = format!("Installing {name}: {} ...", command.join(" "));
                Task::perform(platform::blocking(move || deps::install(&command)), move |result| {
                    Message::DepInstalled(name, result)
                })
            }
            Message::DepInstalled(name, result) => {
                let message = match result {
                    Ok(_) => format!("{name} installed."),
                    Err(e) => format!("{name} failed: {e}"),
                };
                let task = self.check_deps(false);
                if let Some(dialog) = &mut self.deps {
                    dialog.installing = None;
                    dialog.message = message;
                }
                task
            }
            Message::ToggleStartupCheck(on) => {
                self.check_at_startup = on;
                let value = if on { "true" } else { "false" };
                if let Err(e) = platform::write_setting("check_dependencies", value) {
                    self.status = format!("Cannot save settings: {e}");
                }
                Task::none()
            }
            Message::CloseDeps => {
                self.deps = None;
                Task::none()
            }
            Message::CopyText(text) => {
                self.status = "Command copied to the clipboard".into();
                iced::clipboard::write(text)
            }
            Message::OpenUrl(url) => {
                if let Err(e) = platform::open_url(url) {
                    self.status = format!("Cannot open browser: {e}");
                }
                Task::none()
            }
        }
    }

    /// Opens the Dependencies dialog (or refreshes it) and checks the tools.
    fn check_deps(&mut self, startup: bool) -> Task<Message> {
        let os = self.os.unwrap_or_else(Os::detect);
        match &mut self.deps {
            Some(dialog) => dialog.checking = true,
            None => {
                self.deps = Some(DepsDialog {
                    list: Vec::new(),
                    checking: true,
                    installing: None,
                    message: "Checking installed tools...".into(),
                })
            }
        }
        Task::perform(platform::blocking(move || deps::check(os)), move |list| Message::DepsChecked(list, startup))
    }

    // ----- Running programs -----

    fn start_run(&mut self, target: RunTarget) -> Task<Message> {
        if let Some(handle) = self.output.handle.take() {
            handle.kill();
        }
        // For "python script.py" show the script name, otherwise the program.
        let shown = target.args.first().map(PathBuf::from).unwrap_or_else(|| target.program.clone());
        self.output.title = shown.file_name().unwrap_or_default().to_string_lossy().into_owned();
        self.output.screen.clear();
        self.output.input.clear();
        self.output.running = true;
        self.output.visible = !target.gui;
        self.output.run_id += 1;
        self.status = if target.gui {
            "Running (GUI window) - Alt+F5 shows its output".into()
        } else {
            "Running - Alt+F5 toggles the output window".into()
        };

        let run_id = self.output.run_id;
        let run = Task::run(output::run(target), move |event| Message::Output(run_id, event));
        if self.output.visible {
            Task::batch([run, iced::widget::operation::focus(OUTPUT_INPUT)])
        } else {
            run
        }
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

    fn edit(&mut self, action: Action) -> Task<Message> {
        let typed = match &action {
            Action::Edit(Edit::Insert(c)) => Some(*c),
            _ => None,
        };
        let is_edit = action.is_edit();
        self.content.perform(action);

        if is_edit {
            self.modified = true;
            self.diagnostics.clear();
            self.lsp_change();
        }
        if self.completion.is_some() {
            self.refresh_completion();
        }
        if let Some(c) = typed {
            let lang = Lang::from_path(self.path.as_deref());
            let before = self.char_before_cursor(1);
            if c == '.' || (c == ':' && lang == Lang::Rust && before == Some(':')) {
                self.request_completion(Some(c));
            }
        }
        Task::none()
    }

    /// The character `n` places before the one just left of the cursor.
    fn char_before_cursor(&self, n: usize) -> Option<char> {
        let position = self.content.cursor().position;
        let line = self.content.line(position.line)?;
        let column = position.column.checked_sub(n + 1)?;
        line.text.chars().nth(column)
    }

    /// Returns true (and asks) when unsaved edits would be lost and this is
    /// the first press of the command.
    fn needs_confirm(&mut self, previous: Option<Discard>, command: Discard) -> bool {
        if !self.modified || previous == Some(command) {
            return false;
        }
        let key = match command {
            Discard::New => "File > New",
            Discard::Open => "F3",
            Discard::Exit => "Alt+X",
        };
        self.status = format!("Unsaved changes! Press F2 to save, or {key} again to discard.");
        self.confirm = Some(command);
        true
    }

    fn load(&mut self, path: &Path) -> bool {
        match std::fs::read_to_string(path) {
            Ok(source) => {
                self.content = text_editor::Content::with_text(source.trim_start_matches('\u{feff}'));
                self.path = Some(path.to_path_buf());
                self.modified = false;
                self.diagnostics.clear();
                self.set_live(Vec::new());
                self.completion = None;
                self.status = format!("Opened {}", path.display());
                self.lsp_sync_doc();
                true
            }
            Err(e) => {
                self.status = format!("Cannot open {}: {e}", path.display());
                false
            }
        }
    }

    fn save(&mut self, after: AfterSave) -> Task<Message> {
        let Some(path) = self.path.clone() else {
            return save_as_dialog(after);
        };
        if self.modified || !path.exists() {
            if let Err(e) = std::fs::write(&path, self.content.text()) {
                self.status = format!("Cannot save {}: {e}", path.display());
                return Task::none();
            }
            self.modified = false;
            if self.lsp_doc.as_ref() == Some(&path) {
                self.lsp_send(lsp::Request::Save { path: path.clone() });
            }
        }
        self.status = format!("Saved {}", path.display());

        if after == AfterSave::Nothing || self.building {
            return Task::none();
        }
        self.building = true;
        self.diagnostics.clear();
        self.status = format!("Compiling {} ...", path.display());
        Task::perform(platform::blocking(move || runner::build(&path)), move |result| {
            Message::BuildFinished(result, after)
        })
    }

    fn finish_build(&mut self, result: BuildResult, after: AfterSave) -> Task<Message> {
        self.diagnostics = result.diagnostics;
        let errors = self.diagnostics.iter().filter(|d| d.level == "error").count();
        let warnings = self.diagnostics.len() - errors;

        if !result.success {
            self.status = format!("Compile failed: {errors} error(s), {warnings} warning(s)");
            if let Some(first) = self.diagnostics.iter().find(|d| d.level == "error").cloned() {
                self.jump_to(&first);
            }
            return Task::none();
        }

        self.status = format!("Compile successful, {warnings} warning(s)");
        let Some(target) = result.target else { return Task::none() };
        match after {
            AfterSave::Run => self.start_run(target),
            AfterSave::RunExternal => {
                self.status = match runner::run_external(&target) {
                    Ok(()) if target.gui => "Running (GUI window)".into(),
                    Ok(()) => "Running in an external terminal".into(),
                    Err(e) => e,
                };
                Task::none()
            }
            AfterSave::Nothing | AfterSave::Compile => Task::none(),
        }
    }

    fn jump_to(&mut self, diagnostic: &Diagnostic) {
        if diagnostic.line == 0 {
            return;
        }
        if let Some(file) = &diagnostic.file
            && !self.is_current_file(file)
        {
            if self.modified {
                self.status = format!("Save first to open {}", file.display());
                return;
            }
            if !self.load(file) {
                return;
            }
        }
        self.content.move_to(Cursor {
            position: Position {
                line: diagnostic.line - 1,
                column: diagnostic.column.saturating_sub(1),
            },
            selection: None,
        });
    }

    fn is_current_file(&self, file: &Path) -> bool {
        self.path.as_deref().is_some_and(|current| lsp::same_file(current, file))
    }

    /// The Messages panel shows the last build, or live diagnostics when
    /// there is no build output.
    fn panel_items(&self) -> Vec<Diagnostic> {
        if !self.diagnostics.is_empty() {
            return self.diagnostics.clone();
        }
        self.live
            .iter()
            .map(|d| Diagnostic {
                file: self.path.clone(),
                line: d.line + 1,
                column: d.start + 1,
                level: if d.error { "error" } else { "warning" }.into(),
                message: d.message.clone(),
            })
            .collect()
    }

    // ----- Language server -----

    fn lsp_send(&self, request: lsp::Request) {
        if let Some(lsp) = &self.lsp {
            let _ = lsp.unbounded_send(request);
        }
    }

    /// Makes the language server track the file shown in the editor.
    fn lsp_sync_doc(&mut self) {
        if self.lsp.is_none() {
            return;
        }
        let wanted = self
            .path
            .clone()
            .filter(|p| Lang::from_path(Some(p)) != Lang::Plain);
        if wanted == self.lsp_doc {
            return;
        }
        if let Some(old) = self.lsp_doc.take() {
            self.lsp_send(lsp::Request::Close { path: old });
        }
        if let Some(path) = wanted {
            self.lsp_send(lsp::Request::Open { path: path.clone(), text: self.content.text() });
            self.lsp_doc = Some(path);
        }
    }

    fn lsp_change(&self) {
        if let Some(path) = &self.lsp_doc {
            self.lsp_send(lsp::Request::Change { path: path.clone(), text: self.content.text() });
        }
    }

    fn lsp_event(&mut self, event: lsp::Event) {
        match event {
            lsp::Event::Ready(sender) => {
                self.lsp = Some(sender);
                self.lsp_sync_doc();
            }
            lsp::Event::Status(status) => self.lsp_status = status,
            lsp::Event::Diagnostics { path, items } => {
                if self.is_current_file(&path) {
                    self.set_live(items);
                }
            }
            lsp::Event::Completions { id, items } => {
                let Some(completion) = self.completion.as_mut().filter(|c| c.id == id) else {
                    return;
                };
                completion.items = items;
                completion.waiting = false;
                completion.selected = 0;
                self.refresh_completion();
                if self.completion.is_none() {
                    self.status = "No completions".into();
                }
            }
        }
    }

    fn set_live(&mut self, mut items: Vec<lsp::Diagnostic>) {
        // Servers sometimes report the same problem twice (e.g. parser + checker).
        items.dedup_by(|a, b| a.line == b.line && a.start == b.start && a.message == b.message);
        self.marks = Arc::new(
            items
                .iter()
                .map(|d| Mark {
                    line: d.line,
                    start: d.start,
                    end: if d.end_line == d.line { d.end } else { usize::MAX / 2 },
                    error: d.error,
                })
                .collect(),
        );
        self.live = items;
    }

    // ----- Completion -----

    fn request_completion(&mut self, trigger: Option<char>) {
        let Some(path) = self.lsp_doc.clone() else {
            self.status = "No language server for this file".into();
            return;
        };
        let position = self.content.cursor().position;
        let line = self.content.line(position.line).map(|l| l.text.into_owned()).unwrap_or_default();
        let word_len = line
            .chars()
            .take(position.column)
            .collect::<Vec<_>>()
            .iter()
            .rev()
            .take_while(|c| is_ident(**c))
            .count();

        self.next_completion_id += 1;
        self.completion = Some(Completion {
            id: self.next_completion_id,
            anchor: Position { line: position.line, column: position.column - word_len },
            items: Vec::new(),
            selected: 0,
            waiting: true,
        });
        self.lsp_send(lsp::Request::Complete {
            id: self.next_completion_id,
            path,
            line: position.line,
            column: position.column,
            trigger,
        });
    }

    /// The typed part of the word being completed, or None if the cursor left it.
    fn completion_prefix(&self) -> Option<String> {
        let anchor = self.completion.as_ref()?.anchor;
        let position = self.content.cursor().position;
        if position.line != anchor.line || position.column < anchor.column {
            return None;
        }
        let line = self.content.line(position.line)?;
        let prefix: String =
            line.text.chars().skip(anchor.column).take(position.column - anchor.column).collect();
        prefix.chars().all(is_ident).then_some(prefix)
    }

    fn visible_completions(&self) -> Vec<&lsp::CompletionItem> {
        let (Some(completion), Some(prefix)) = (&self.completion, self.completion_prefix()) else {
            return Vec::new();
        };
        let prefix = prefix.to_lowercase();
        completion
            .items
            .iter()
            .filter(|item| item.filter.to_lowercase().starts_with(&prefix))
            .collect()
    }

    fn refresh_completion(&mut self) {
        let waiting = self.completion.as_ref().is_some_and(|c| c.waiting);
        let visible = self.visible_completions().len();
        if self.completion_prefix().is_none() || (!waiting && visible == 0) {
            self.completion = None;
        } else if let Some(completion) = &mut self.completion {
            completion.selected = completion.selected.min(visible.saturating_sub(1));
        }
    }

    fn completion_nav(&mut self, nav: Nav) {
        let visible = self.visible_completions().len();
        let Some(completion) = &mut self.completion else {
            return;
        };
        match nav {
            Nav::Up => completion.selected = completion.selected.saturating_sub(1),
            Nav::Down => completion.selected = (completion.selected + 1).min(visible.saturating_sub(1)),
            Nav::Cancel => self.completion = None,
            Nav::Accept => {
                let selected = completion.selected;
                self.accept_completion(selected);
            }
        }
    }

    fn accept_completion(&mut self, index: usize) {
        let Some(item) = self.visible_completions().get(index).map(|item| item.insert.clone())
        else {
            self.completion = None;
            return;
        };
        let typed = self.completion_prefix().map_or(0, |p| p.chars().count());
        self.completion = None;
        for _ in 0..typed {
            self.content.perform(Action::Edit(Edit::Backspace));
        }
        self.content.perform(Action::Edit(Edit::Paste(Arc::new(item))));
        self.modified = true;
        self.diagnostics.clear();
        self.lsp_change();
    }

    // ----- View -----

    fn view(&self) -> Element<'_, Message> {
        let menu_bar = container(row(MENU_TITLES.iter().map(|&title| {
            let menu = match title {
                "File" => Some(Menu::File),
                "Run" => Some(Menu::Run),
                "Compile" => Some(Menu::Compile),
                "Options" => Some(Menu::Options),
                "Help" => Some(Menu::Help),
                _ => None,
            };
            let open = menu.is_some() && menu == self.menu;
            button(text(title).color(Color::BLACK))
                .width(MENU_TITLE_WIDTH)
                .padding([4, 10])
                .on_press_maybe(menu.map(Message::ToggleMenu))
                .style(move |_, status| menu_title_style(open, status))
                .into()
        })))
        .width(Length::Fill)
        .style(|_| container::Style::default().background(TURBO_GRAY));

        let popup_open = self.completion.is_some();
        let editor = text_editor(&self.content)
            .on_action(Message::Edit)
            .height(Length::Fill)
            .highlight_with::<highlight::Highlighter>(
                highlight::Settings {
                    lang: Lang::from_path(self.path.as_deref()),
                    marks: self.marks.clone(),
                },
                highlight::to_format,
            )
            .key_binding(move |press: KeyPress| {
                if popup_open && let Some(nav) = completion_key(&press.key) {
                    return Some(Binding::Custom(Message::CompletionNav(nav)));
                }
                shortcut(&press.key, press.physical_key, press.modifiers)
                    .map(Binding::Custom)
                    .or_else(|| Binding::from_key_press(press))
            })
            .style(|theme, status| text_editor::Style {
                background: Background::Color(TURBO_BLUE),
                value: TURBO_YELLOW,
                ..text_editor::default(theme, status)
            });

        let panel = self.panel_items();
        let mut layout = column![menu_bar, editor];
        if !panel.is_empty() {
            layout = layout.push(self.messages_panel(panel));
        }
        layout = layout.push(self.status_bar());

        let mut layers = stack![layout];
        if let Some(popup) = self.completion_popup() {
            let bottom = STATUS_BAR_HEIGHT
                + if self.panel_items().is_empty() { 0.0 } else { MESSAGES_HEIGHT };
            layers = layers.push(
                container(popup)
                    .align_right(Length::Fill)
                    .align_bottom(Length::Fill)
                    .padding(Padding::default().bottom(bottom + 4.0).right(16.0)),
            );
        }
        if self.output.visible {
            layers = layers.push(self.output_window());
        }
        if let Some(dialog) = &self.deps {
            layers = layers.push(self.deps_dialog(dialog));
        }
        if let Some(menu) = self.menu {
            layers = layers
                .push(
                    mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                        .on_press(Message::CloseMenu),
                )
                .push(dropdown(menu));
        }
        layers.into()
    }

    /// The program's screen, drawn over the editor like Turbo Pascal's User screen.
    fn output_window(&self) -> Element<'_, Message> {
        let state = if self.output.running { "running" } else { "finished" };
        let title = text(format!(" Output - {} ({state}) ", self.output.title)).color(Color::BLACK);
        let title_bar = container(row![
            title,
            Space::new().width(Length::Fill),
            small_button("Stop", (self.output.running).then_some(Message::OutputStop)),
            small_button("Hide", Some(Message::OutputToggle)),
        ])
        .width(Length::Fill)
        .style(|_| container::Style::default().background(TURBO_GRAY));

        let screen = scrollable(
            text(&self.output.screen).color(SCREEN_TEXT).font(Font::MONOSPACE).width(Length::Fill),
        )
        .anchor_bottom()
        .height(Length::Fill);

        let input = text_input(
            if self.output.running { "Type input for the program, Enter to send" } else { "" },
            &self.output.input,
        )
        .id(OUTPUT_INPUT)
        .on_input_maybe(self.output.running.then_some(Message::OutputInput))
        .on_submit(Message::OutputSubmit)
        .font(Font::MONOSPACE);

        let footer = text(" Enter Send input   Ctrl+F2 Stop   Alt+F5/Esc Hide").color(Color::BLACK);
        let window = container(column![
            title_bar,
            container(screen).padding([4, 8]).height(Length::Fill),
            input,
            container(footer).width(Length::Fill).style(|_| {
                container::Style::default().background(TURBO_GRAY)
            }),
        ])
        .style(|_| container::Style {
            border: iced::Border::default().color(TURBO_GRAY).width(2),
            ..container::Style::default().background(Color::BLACK)
        });

        opaque(
            container(window)
                .padding(Padding::default().top(MENU_BAR_HEIGHT + 8.0).bottom(STATUS_BAR_HEIGHT + 8.0).left(24.0).right(24.0))
                .width(Length::Fill)
                .height(Length::Fill),
        )
    }

    fn deps_dialog<'a>(&'a self, dialog: &'a DepsDialog) -> Element<'a, Message> {
        let busy = dialog.checking || dialog.installing.is_some();
        let rows = dialog.list.iter().enumerate().map(|(index, dep)| {
            let (mark, color) = match (&dep.found, dep.need) {
                (Some(_), _) => ("[ OK ]", TURBO_GREEN),
                (None, deps::Need::Optional) => ("[ -- ]", TURBO_DARK_GRAY),
                (None, _) => ("[MISS]", TURBO_RED),
            };
            let need = match dep.need {
                deps::Need::Required => "required",
                deps::Need::Recommended => "recommended",
                deps::Need::Optional => "optional",
            };
            let detail: Element<'a, Message> = match &dep.found {
                Some(found) => text(truncate(found, 52)).color(TURBO_DARK_GRAY).into(),
                None => row![
                    text(&dep.install).color(TURBO_BLUE).width(Length::Fill),
                    small_button(
                        "Install",
                        dep.auto.as_ref().filter(|_| !busy).map(|_| Message::DepInstall(index)),
                    ),
                    small_button("Copy", Some(Message::CopyText(dep.install.clone()))),
                ]
                .spacing(6)
                .into(),
            };
            row![
                text(mark).color(color).width(64),
                column![
                    text(dep.name).color(Color::BLACK),
                    text(format!("{} ({need})", dep.purpose)).color(TURBO_DARK_GRAY).size(12),
                ]
                .width(300),
                detail,
            ]
            .spacing(8)
            .into()
        });

        let os = self.os.map_or("", Os::name);
        let header = text(format!(" Dependencies - {os} ")).color(Color::WHITE);
        let message = if dialog.checking && dialog.list.is_empty() {
            "Checking installed tools...".to_string()
        } else {
            dialog.message.clone()
        };
        let buttons = row![
            checkbox(self.check_at_startup)
                .label("Check at startup")
                .on_toggle(Message::ToggleStartupCheck)
                .style(|theme, status| iced::widget::checkbox::Style {
                    text_color: Some(Color::BLACK),
                    ..iced::widget::checkbox::primary(theme, status)
                }),
            Space::new().width(Length::Fill),
            small_button("Install guide on GitHub", Some(Message::OpenUrl(platform::INSTALL_GUIDE_URL))),
            small_button("Re-check", (!busy).then_some(Message::ShowDeps)),
            small_button("Close", Some(Message::CloseDeps)),
        ]
        .spacing(8);

        let body = column![
            container(header).width(Length::Fill).padding(2).style(|_| {
                container::Style::default().background(TURBO_BLUE)
            }),
            column(rows).spacing(10).padding(10),
            container(text(message).color(TURBO_RED)).padding([0, 10]),
            container(buttons).padding(10),
        ];

        opaque(
            container(
                container(body).width(900).style(|_| container::Style {
                    border: iced::Border::default().color(Color::BLACK).width(1),
                    ..container::Style::default().background(TURBO_GRAY)
                }),
            )
            .center(Length::Fill),
        )
    }

    fn completion_popup(&self) -> Option<Element<'_, Message>> {
        let completion = self.completion.as_ref()?;
        let visible = self.visible_completions();
        if visible.is_empty() {
            return completion
                .waiting
                .then(|| popup_frame(text(" Searching... ").color(Color::BLACK).into()));
        }

        let first = completion.selected.saturating_sub(COMPLETION_ROWS - 1);
        let rows = visible
            .iter()
            .enumerate()
            .skip(first)
            .take(COMPLETION_ROWS)
            .map(|(index, item)| {
                let selected = index == completion.selected;
                let detail: String = item.detail.chars().take(40).collect();
                button(row![
                    text(format!("{:<8}", item.kind)).color(TURBO_DARK_GRAY),
                    text(&item.label).color(Color::BLACK),
                    Space::new().width(Length::Fill),
                    text(detail).color(TURBO_DARK_GRAY),
                ])
                .width(Length::Fill)
                .padding([1, 6])
                .on_press(Message::CompletionPick(index))
                .style(move |_, status| button::Style {
                    background: (selected || matches!(status, button::Status::Hovered))
                        .then(|| Background::Color(TURBO_GREEN)),
                    ..button::Style::default()
                })
                .into()
            });

        let header = text(format!(
            " Completions {}/{}   Up/Down  Enter/Tab  Esc",
            completion.selected + 1,
            visible.len()
        ))
        .color(Color::BLACK);
        Some(popup_frame(column![header, column(rows)].into()))
    }

    fn messages_panel(&self, items: Vec<Diagnostic>) -> Element<'_, Message> {
        let title = if self.diagnostics.is_empty() { " Messages (live) " } else { " Messages " };
        let rows = items.into_iter().enumerate().map(|(index, d)| {
            let location = match (&d.file, d.line) {
                (Some(file), line) if line > 0 => format!(
                    "{}({line},{}): ",
                    file.file_name().unwrap_or_default().to_string_lossy(),
                    d.column
                ),
                _ => String::new(),
            };
            let color = if d.level == "error" { TURBO_RED } else { Color::BLACK };
            button(text(format!("{} {location}{}", d.level.to_uppercase(), d.message)).color(color))
                .width(Length::Fill)
                .padding([1, 8])
                .on_press(Message::JumpTo(index))
                .style(|_, status| button::Style {
                    background: matches!(status, button::Status::Hovered)
                        .then(|| Background::Color(TURBO_GREEN)),
                    text_color: Color::BLACK,
                    ..button::Style::default()
                })
                .into()
        });

        container(column![
            text(title).color(Color::WHITE),
            scrollable(column(rows)).height(Length::Fill),
        ])
        .height(MESSAGES_HEIGHT)
        .width(Length::Fill)
        .style(|_| container::Style::default().background(TURBO_CYAN))
        .into()
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let cursor = self.content.cursor().position;
        let keys = " F2 Save  F3 Open  F5 Run  Alt+F5 Output  F9 Compile";
        // A live diagnostic on the cursor line takes priority over the last status.
        let message = self
            .live
            .iter()
            .find(|d| d.line == cursor.line)
            .map_or(self.status.as_str(), |d| d.message.as_str());
        container(row![
            text(keys).color(Color::BLACK),
            // The message takes the remaining space and is clipped, so the
            // server status and cursor position always stay visible.
            container(text(format!("   {message}")).color(TURBO_RED).wrapping(text::Wrapping::None))
                .width(Length::Fill)
                .clip(true),
            text(format!("  {}  ", truncate(&self.lsp_status, 30))).color(TURBO_DARK_GRAY),
            text(format!("{}:{} ", cursor.line + 1, cursor.column + 1)).color(Color::BLACK),
        ])
        .padding([2, 4])
        .height(STATUS_BAR_HEIGHT)
        .width(Length::Fill)
        .style(|_| container::Style::default().background(TURBO_GRAY))
        .into()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut short: String = s.chars().take(max - 3).collect();
    short.push_str("...");
    short
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Global shortcuts, Turbo Pascal style (plus the usual Ctrl+S / Ctrl+O).
fn shortcut(key: &Key, physical: key::Physical, modifiers: Modifiers) -> Option<Message> {
    match key.as_ref() {
        Key::Named(key::Named::F2) if modifiers.control() => Some(Message::OutputStop),
        Key::Named(key::Named::F2) => Some(Message::Save),
        Key::Named(key::Named::F3) => Some(Message::Open),
        Key::Named(key::Named::F5) if modifiers.alt() => Some(Message::OutputToggle),
        Key::Named(key::Named::F5) => Some(Message::Run),
        Key::Named(key::Named::F9) if modifiers.control() => Some(Message::Run),
        Key::Named(key::Named::F9) => Some(Message::Compile),
        Key::Named(key::Named::Space) if modifiers.control() => Some(Message::TriggerCompletion),
        _ => match key.to_latin(physical) {
            Some('x') if modifiers.alt() => Some(Message::Exit),
            Some('s') if modifiers.command() => Some(Message::Save),
            Some('o') if modifiers.command() => Some(Message::Open),
            _ => None,
        },
    }
}

fn completion_key(key: &Key) -> Option<Nav> {
    match key.as_ref() {
        Key::Named(key::Named::ArrowUp) => Some(Nav::Up),
        Key::Named(key::Named::ArrowDown) => Some(Nav::Down),
        Key::Named(key::Named::Enter | key::Named::Tab) => Some(Nav::Accept),
        Key::Named(key::Named::Escape) => Some(Nav::Cancel),
        _ => None,
    }
}

fn popup_frame(content: Element<'_, Message>) -> Element<'_, Message> {
    container(content)
        .width(460)
        .padding(2)
        .style(|_| container::Style {
            border: iced::Border::default().color(Color::BLACK).width(1),
            ..container::Style::default().background(TURBO_CYAN)
        })
        .into()
}

fn save_as_dialog(after: AfterSave) -> Task<Message> {
    Task::perform(
        rfd::AsyncFileDialog::new()
            .set_title("Save As")
            .add_filter("Rust", &["rs"])
            .add_filter("Python", &["py"])
            .add_filter("All files", &["*"])
            .save_file(),
        move |file| Message::SaveAsPicked(file.map(|f| f.path().to_path_buf()), after),
    )
}

fn dropdown(menu: Menu) -> Element<'static, Message> {
    let (index, items): (usize, &[(&str, &str, Message)]) = match menu {
        Menu::File => (
            0,
            &[
                ("New", "", Message::New),
                ("Open...", "F3", Message::Open),
                ("Save", "F2", Message::Save),
                ("Save as...", "", Message::SaveAs),
                ("Exit", "Alt+X", Message::Exit),
            ],
        ),
        Menu::Run => (
            3,
            &[
                ("Run", "F5", Message::Run),
                ("Run in external terminal", "", Message::RunExternal),
                ("Program reset", "Ctrl+F2", Message::OutputStop),
                ("User screen", "Alt+F5", Message::OutputToggle),
            ],
        ),
        Menu::Compile => (4, &[("Compile", "F9", Message::Compile)]),
        Menu::Options => (6, &[("Dependencies...", "", Message::ShowDeps)]),
        Menu::Help => (
            8,
            &[
                ("Install guide (GitHub)", "", Message::OpenUrl(platform::INSTALL_GUIDE_URL)),
                ("Project page (GitHub)", "", Message::OpenUrl(platform::GITHUB_URL)),
            ],
        ),
    };

    let entries = items.iter().map(|(label, keys, message)| {
        button(row![
            text(*label).color(Color::BLACK),
            Space::new().width(Length::Fill),
            text(*keys).color(Color::BLACK),
        ])
        .width(290)
        .padding([3, 10])
        .on_press(message.clone())
        .style(|_, status| button::Style {
            background: Some(Background::Color(
                if matches!(status, button::Status::Hovered) { TURBO_GREEN } else { TURBO_GRAY },
            )),
            ..button::Style::default()
        })
        .into()
    });

    container(container(column(entries)).style(|_| container::Style {
        border: iced::Border::default().color(Color::BLACK).width(1),
        ..container::Style::default().background(TURBO_GRAY)
    }))
    .padding(Padding::default().top(MENU_BAR_HEIGHT).left(index as f32 * MENU_TITLE_WIDTH))
    .into()
}

/// A Turbo-style dialog button; disabled when `message` is None.
fn small_button(label: &str, message: Option<Message>) -> Element<'_, Message> {
    let enabled = message.is_some();
    button(text(format!(" {label} ")).color(if enabled { Color::BLACK } else { TURBO_DARK_GRAY }))
        .padding([1, 6])
        .on_press_maybe(message)
        .style(move |_, status| button::Style {
            background: Some(Background::Color(
                if enabled && matches!(status, button::Status::Hovered) { TURBO_CYAN } else { TURBO_GREEN },
            )),
            text_color: Color::BLACK,
            ..button::Style::default()
        })
        .into()
}

fn menu_title_style(open: bool, status: button::Status) -> button::Style {
    let highlighted = open || matches!(status, button::Status::Hovered);
    button::Style {
        background: highlighted.then(|| Background::Color(TURBO_GREEN)),
        text_color: Color::BLACK,
        ..button::Style::default()
    }
}
