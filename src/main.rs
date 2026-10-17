//! rust-ide: a Turbo Pascal–style IDE for Rust and Python (iced GUI front end).

// Release builds are GUI apps on Windows: no console window when started.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod deps;
mod highlight;
mod lsp;
mod output;
mod platform;
mod runner;
mod search;
mod vim;

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

use highlight::{Lang, Mark, MarkKind};
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
const SEARCH_INPUT: &str = "search-input";
const EDITOR: &str = "editor";

const USAGE: &str = "\
Usage: rust-ide [OPTIONS] [FILE]

Turbo Pascal-style IDE for Rust and Python.

Options:
  --software   use software rendering (for broken GPU drivers)
  --version    print the version and exit
  --help       print this help and exit

Install guide: https://github.com/V3ct0r-8890/rust-ide/blob/main/docs/INSTALL.md";

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let info = args.iter().any(|a| matches!(a.as_str(), "--version" | "-V" | "--help" | "-h"));
    #[cfg(windows)]
    if info {
        // A GUI-subsystem exe has no console; borrow the terminal it was started from.
        unsafe extern "system" {
            fn AttachConsole(process_id: u32) -> i32;
        }
        // SAFETY: plain Win32 call; ATTACH_PARENT_PROCESS = u32::MAX.
        unsafe { AttachConsole(u32::MAX) };
    }
    let _ = info;
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("rust-ide {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
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
    Edit,
    Search,
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
    /// Vim emulation, when enabled in Options.
    vim: Option<vim::Vim>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// True while consecutive typing is being collected into one undo step.
    undo_group: bool,
    /// The Find / Replace / Go to line bar, when open.
    search: Option<SearchBar>,
    /// The last searched text and case setting, for Search again (Ctrl+L).
    last_query: String,
    last_case: bool,
    /// Highlights for the matches of the open search bar.
    search_marks: Arc<Vec<Mark>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SearchMode {
    Find,
    Replace,
    GoTo,
}

struct SearchBar {
    mode: SearchMode,
    query: String,
    replacement: String,
    case: bool,
}

#[derive(Clone)]
struct Snapshot {
    text: String,
    cursor: Cursor,
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
    Vim(vim::Key),
    Undo,
    Redo,
    ToggleVim,
    OpenSearch(SearchMode),
    SearchQuery(String),
    SearchReplacement(String),
    SearchCase(bool),
    FindNext,
    FindPrevious,
    ReplaceOne,
    ReplaceAll,
    GoToLine,
    CloseSearch,
}

impl Ide {
    /// Starts empty, or with the file given on the command line, and checks
    /// the external tools in the background.
    fn new() -> (Self, Task<Message>) {
        let mut ide = Self::default();
        let os = Os::detect();
        ide.os = Some(os);
        if platform::read_setting("vim").as_deref() == Some("true") {
            ide.vim = Some(vim::Vim::default());
        }
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
            Message::Vim(key) => self.vim_key(key),
            Message::Undo => {
                self.undo();
                Task::none()
            }
            Message::Redo => {
                self.redo();
                Task::none()
            }
            Message::ToggleVim => {
                self.vim = match self.vim {
                    Some(_) => None,
                    None => Some(vim::Vim::default()),
                };
                let on = self.vim.is_some();
                self.status = format!("Vim mode {}", if on { "on" } else { "off" });
                if let Err(e) = platform::write_setting("vim", if on { "true" } else { "false" }) {
                    self.status = format!("Cannot save settings: {e}");
                }
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
                } else if self.search.is_some() {
                    return self.close_search();
                }
                Task::none()
            }
            Message::OpenSearch(mode) => self.open_search(mode),
            Message::SearchQuery(query) => {
                if let Some(bar) = &mut self.search {
                    bar.query = query;
                }
                self.refresh_search_marks();
                Task::none()
            }
            Message::SearchReplacement(replacement) => {
                if let Some(bar) = &mut self.search {
                    bar.replacement = replacement;
                }
                Task::none()
            }
            Message::SearchCase(case) => {
                if let Some(bar) = &mut self.search {
                    bar.case = case;
                }
                self.refresh_search_marks();
                Task::none()
            }
            Message::FindNext => {
                self.find(true);
                Task::none()
            }
            Message::FindPrevious => {
                self.find(false);
                Task::none()
            }
            Message::ReplaceOne => {
                self.replace_one();
                Task::none()
            }
            Message::ReplaceAll => {
                self.replace_all();
                Task::none()
            }
            Message::GoToLine => self.go_to_line(),
            Message::CloseSearch => self.close_search(),
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
        // Typing is undone in groups: a group starts at the first edit after
        // the cursor was moved, and each new line starts a new group.
        if is_edit && !self.undo_group {
            self.push_undo();
            self.undo_group = true;
        }
        let new_line = matches!(action, Action::Edit(Edit::Enter));
        if !is_edit || new_line {
            self.undo_group = false;
        }
        self.content.perform(action);

        if is_edit {
            self.modified = true;
            self.diagnostics.clear();
            self.lsp_change();
            if !self.search_marks.is_empty() {
                self.refresh_search_marks();
            }
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
        line.text.get(..position.column)?.chars().rev().nth(n)
    }

    // ----- Undo -----

    /// Records the buffer before a change.
    fn push_undo(&mut self) {
        const LIMIT: usize = 200;
        self.undo.push(Snapshot { text: self.content.text(), cursor: self.content.cursor() });
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn undo(&mut self) {
        let Some(snapshot) = self.undo.pop() else {
            self.status = "Nothing to undo".into();
            return;
        };
        let current = Snapshot { text: self.content.text(), cursor: self.content.cursor() };
        self.redo.push(current);
        self.restore(snapshot);
    }

    fn redo(&mut self) {
        let Some(snapshot) = self.redo.pop() else {
            self.status = "Nothing to redo".into();
            return;
        };
        let current = Snapshot { text: self.content.text(), cursor: self.content.cursor() };
        self.undo.push(current);
        self.restore(snapshot);
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.content = text_editor::Content::with_text(&snapshot.text);
        vim::place(&mut self.content, snapshot.cursor.position);
        self.undo_group = false;
        self.completion = None;
        self.modified = true;
        self.diagnostics.clear();
        self.lsp_change();
    }

    // ----- Vim -----

    // ----- Search -----

    fn open_search(&mut self, mode: SearchMode) -> Task<Message> {
        // Start from the selected text (one line only), else the last search.
        let selected = self.content.selection().filter(|s| !s.contains('\n') && !s.is_empty());
        let query = match mode {
            SearchMode::GoTo => String::new(),
            _ => selected.unwrap_or_else(|| self.last_query.clone()),
        };
        let replacement = self.search.as_ref().map(|b| b.replacement.clone()).unwrap_or_default();
        self.search = Some(SearchBar { mode, query, replacement, case: self.last_case });
        Task::batch([
            iced::widget::operation::focus(SEARCH_INPUT),
            iced::widget::operation::move_cursor_to_end(SEARCH_INPUT),
        ])
    }

    fn close_search(&mut self) -> Task<Message> {
        // Remember what was typed, so Search again (Ctrl+L) uses it.
        if let Some(bar) = self.search.take()
            && bar.mode != SearchMode::GoTo
            && !bar.query.is_empty()
        {
            self.last_query = bar.query;
            self.last_case = bar.case;
        }
        self.search_marks = Arc::default();
        iced::widget::operation::focus(EDITOR)
    }

    /// The text to search for: the open bar's, or the last one (Ctrl+L).
    fn search_query(&mut self) -> Option<(String, bool)> {
        if let Some(bar) = &self.search
            && bar.mode != SearchMode::GoTo
        {
            self.last_query = bar.query.clone();
            self.last_case = bar.case;
        }
        if self.last_query.is_empty() {
            self.status = "Nothing to search for (Ctrl+F)".into();
            return None;
        }
        Some((self.last_query.clone(), self.last_case))
    }

    fn find(&mut self, forward: bool) {
        let Some((query, case)) = self.search_query() else { return };
        let text = self.content.text();
        let cursor = self.content.cursor();
        let here = search::offset_of(&text, cursor.position);
        let other = cursor.selection.map_or(here, |s| search::offset_of(&text, s));
        let found = if forward {
            search::find(&text, &query, here.max(other), case)
        } else {
            search::find_back(&text, &query, here.min(other), case)
        };
        let Some((start, end, wrapped)) = found else {
            self.status = format!("Search string not found: {query}");
            return;
        };
        self.content.move_to(Cursor {
            position: search::position_of(&text, end),
            selection: Some(search::position_of(&text, start)),
        });
        let matches = search::all(&text, &query, case);
        self.mark_matches(&text, &matches, Some(start));
        let index = matches.iter().position(|&(s, _)| s == start).map_or(0, |i| i + 1);
        let wrapped = if wrapped { " (wrapped)" } else { "" };
        self.status = format!("Match {index} of {}{wrapped}", matches.len());
    }

    fn replace_one(&mut self) {
        let Some((query, case)) = self.search_query() else { return };
        let replacement = self.search.as_ref().map(|b| b.replacement.clone()).unwrap_or_default();
        let selected = self.content.selection().unwrap_or_default();
        if search::is_match(&selected, &query, case) {
            self.push_undo();
            self.undo_group = false;
            self.content.perform(Action::Edit(Edit::Paste(Arc::new(replacement))));
            self.modified = true;
            self.diagnostics.clear();
            self.lsp_change();
        }
        self.find(true);
    }

    fn replace_all(&mut self) {
        let Some((query, case)) = self.search_query() else { return };
        let replacement = self.search.as_ref().map(|b| b.replacement.clone()).unwrap_or_default();
        let text = self.content.text();
        let (new_text, count) = search::replace_all(&text, &query, &replacement, case);
        if count == 0 {
            self.status = format!("Search string not found: {query}");
            return;
        }
        let position = self.content.cursor().position;
        self.push_undo();
        self.undo_group = false;
        self.content = text_editor::Content::with_text(&new_text);
        self.place_clamped(position.line, position.column);
        self.modified = true;
        self.diagnostics.clear();
        self.lsp_change();
        self.status = format!("Replaced {count} occurrence(s)");
        self.refresh_search_marks();
    }

    /// Highlights `matches` (byte ranges); `current` is the selected one.
    fn mark_matches(&mut self, text: &str, matches: &[(usize, usize)], current: Option<usize>) {
        let column = |offset: usize| {
            let p = search::position_of(text, offset);
            let line_start = offset - p.column;
            (p.line, text[line_start..offset].chars().count())
        };
        self.search_marks = Arc::new(
            matches
                .iter()
                .map(|&(start, end)| {
                    let (line, from) = column(start);
                    let (_, to) = column(end);
                    let kind = if Some(start) == current { MarkKind::Current } else { MarkKind::Match };
                    Mark { line, start: from, end: to, kind }
                })
                .collect(),
        );
    }

    /// Re-marks the open bar's matches after the text or query changed.
    fn refresh_search_marks(&mut self) {
        let Some(bar) = &self.search else { return };
        if bar.mode == SearchMode::GoTo || bar.query.is_empty() {
            self.search_marks = Arc::default();
            return;
        }
        let (query, case) = (bar.query.clone(), bar.case);
        let text = self.content.text();
        let matches = search::all(&text, &query, case);
        self.mark_matches(&text, &matches, None);
    }

    fn go_to_line(&mut self) -> Task<Message> {
        let query = self.search.as_ref().map(|b| b.query.trim().to_string()).unwrap_or_default();
        match query.parse::<usize>() {
            Ok(line) if line >= 1 => {
                self.place_clamped(line - 1, 0);
                self.close_search()
            }
            _ => {
                self.status = format!("Enter a line number from 1 to {}", self.content.line_count());
                Task::none()
            }
        }
    }

    /// Moves the cursor, keeping the position inside the text.
    fn place_clamped(&mut self, line: usize, column: usize) {
        let line = line.min(self.content.line_count().saturating_sub(1));
        let text = self.content.line(line).map(|l| l.text.into_owned()).unwrap_or_default();
        let mut column = column.min(text.len());
        while !text.is_char_boundary(column) {
            column -= 1;
        }
        vim::place(&mut self.content, Position { line, column });
    }

    fn search_bar<'a>(&'a self, bar: &'a SearchBar) -> Element<'a, Message> {
        let label = |s: &'static str| text(s).color(Color::BLACK);
        let input = |value: &'a str, on_input: fn(String) -> Message, submit: Message, width: f32| {
            text_input("", value)
                .on_input(on_input)
                .on_submit(submit)
                .font(Font::MONOSPACE)
                .width(width)
                .padding([2, 6])
        };

        let close = small_button("Close", Some(Message::CloseSearch));
        let rows: Element<'a, Message> = match bar.mode {
            SearchMode::GoTo => row![
                label(" Go to line:"),
                input(&bar.query, Message::SearchQuery, Message::GoToLine, 100.0).id(SEARCH_INPUT),
                small_button("Go", Some(Message::GoToLine)),
                text(format!("(1-{})", self.content.line_count())).color(TURBO_DARK_GRAY),
                Space::new().width(Length::Fill),
                close,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into(),
            mode => {
                let find = row![
                    label(" Find:        "),
                    input(&bar.query, Message::SearchQuery, Message::FindNext, 260.0).id(SEARCH_INPUT),
                    checkbox(bar.case)
                        .label("Case")
                        .on_toggle(Message::SearchCase)
                        .style(|theme, status| iced::widget::checkbox::Style {
                            text_color: Some(Color::BLACK),
                            ..iced::widget::checkbox::primary(theme, status)
                        }),
                    small_button("Next", Some(Message::FindNext)),
                    small_button("Previous", Some(Message::FindPrevious)),
                    Space::new().width(Length::Fill),
                    close,
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center);
                if mode == SearchMode::Replace {
                    let replace = row![
                        label(" Replace with:"),
                        input(&bar.replacement, Message::SearchReplacement, Message::ReplaceOne, 260.0),
                        small_button("Replace", Some(Message::ReplaceOne)),
                        small_button("Replace all", Some(Message::ReplaceAll)),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center);
                    column![find, replace].spacing(4).into()
                } else {
                    find.into()
                }
            }
        };

        container(rows)
            .padding([3, 6])
            .width(Length::Fill)
            .style(|_| container::Style {
                border: iced::Border::default().color(Color::BLACK).width(1),
                ..container::Style::default().background(TURBO_GRAY)
            })
            .into()
    }

    fn vim_key(&mut self, key: vim::Key) -> Task<Message> {
        let Some(vim) = &mut self.vim else { return Task::none() };
        let before = Snapshot { text: self.content.text(), cursor: self.content.cursor() };
        let effects = vim.key(key, &mut self.content);
        self.undo_group = false;

        let mut tasks = Vec::new();
        for effect in effects {
            match effect {
                vim::Effect::Edited => {
                    self.undo.push(before.clone());
                    self.redo.clear();
                    self.modified = true;
                    self.diagnostics.clear();
                    self.lsp_change();
                }
                vim::Effect::Undo => self.undo(),
                vim::Effect::Redo => self.redo(),
                vim::Effect::Status(status) => self.status = status,
                vim::Effect::Save => tasks.push(self.save(AfterSave::Nothing)),
                vim::Effect::Quit if self.modified => {
                    self.status = "No write since last change (add ! to override)".into();
                }
                vim::Effect::Quit | vim::Effect::ForceQuit => tasks.push(iced::exit()),
                vim::Effect::SaveQuit => {
                    if self.path.is_none() {
                        self.status = "No file name (use F2 to save first)".into();
                    } else {
                        tasks.push(self.save(AfterSave::Nothing));
                        if !self.modified {
                            tasks.push(iced::exit());
                        }
                    }
                }
            }
        }
        Task::batch(tasks)
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
        // Diagnostics count characters; the editor counts bytes.
        let line = diagnostic.line - 1;
        let text = self.content.line(line).map(|l| l.text.into_owned()).unwrap_or_default();
        let column = text
            .char_indices()
            .nth(diagnostic.column.saturating_sub(1))
            .map_or(text.len(), |(i, _)| i);
        vim::place(&mut self.content, Position { line, column });
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
                    kind: if d.error { MarkKind::Error } else { MarkKind::Warning },
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
        let before = line.get(..position.column).unwrap_or(&line);
        let word_start = before.trim_end_matches(is_ident).len(); // bytes

        self.next_completion_id += 1;
        self.completion = Some(Completion {
            id: self.next_completion_id,
            anchor: Position { line: position.line, column: word_start },
            items: Vec::new(),
            selected: 0,
            waiting: true,
        });
        self.lsp_send(lsp::Request::Complete {
            id: self.next_completion_id,
            path,
            line: position.line,
            column: before.chars().count(), // the LSP client works in characters
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
        let prefix = line.text.get(anchor.column..position.column)?.to_string();
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
        self.push_undo();
        self.undo_group = false;
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
                "Edit" => Some(Menu::Edit),
                "Search" => Some(Menu::Search),
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
        let vim_mode = self.vim.as_ref().map(|v| (v.mode, v.command.is_some()));
        let editor = text_editor(&self.content)
            .id(EDITOR)
            .on_action(Message::Edit)
            .height(Length::Fill)
            .highlight_with::<highlight::Highlighter>(
                highlight::Settings {
                    lang: Lang::from_path(self.path.as_deref()),
                    marks: if self.search_marks.is_empty() {
                        self.marks.clone()
                    } else {
                        // Search highlights are drawn over diagnostics.
                        Arc::new(self.marks.iter().chain(self.search_marks.iter()).cloned().collect())
                    },
                },
                highlight::to_format,
            )
            .key_binding(move |press: KeyPress| {
                if popup_open && let Some(nav) = completion_key(&press.key) {
                    return Some(Binding::Custom(Message::CompletionNav(nav)));
                }
                if let Some(message) = shortcut(&press.key, press.physical_key, press.modifiers) {
                    return Some(Binding::Custom(message));
                }
                if let Some((mode, command)) = vim_mode
                    && let Some(key) = vim_binding(&press, mode, command)
                {
                    return Some(Binding::Custom(Message::Vim(key)));
                }
                Binding::from_key_press(press)
            })
            .style(|theme, status| text_editor::Style {
                background: Background::Color(TURBO_BLUE),
                value: TURBO_YELLOW,
                ..text_editor::default(theme, status)
            });

        let panel = self.panel_items();
        let mut layout = column![menu_bar];
        if let Some(bar) = &self.search {
            layout = layout.push(self.search_bar(bar));
        }
        layout = layout.push(editor);
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
                .push(dropdown(menu, self.vim.is_some()));
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

    /// The cursor column in characters (the editor reports bytes).
    fn cursor_char_column(&self) -> usize {
        let position = self.content.cursor().position;
        self.content
            .line(position.line)
            .and_then(|l| l.text.get(..position.column).map(|s| s.chars().count()))
            .unwrap_or(position.column)
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
            text(self.vim.as_ref().map(|v| format!("{}  ", v.status())).unwrap_or_default())
                .color(TURBO_BLUE),
            text(format!("{}:{} ", cursor.line + 1, self.cursor_char_column() + 1)).color(Color::BLACK),
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
            Some('z') if modifiers.command() && modifiers.shift() => Some(Message::Redo),
            Some('z') if modifiers.command() => Some(Message::Undo),
            Some('y') if modifiers.command() => Some(Message::Redo),
            Some('f') if modifiers.command() => Some(Message::OpenSearch(SearchMode::Find)),
            Some('h') if modifiers.command() => Some(Message::OpenSearch(SearchMode::Replace)),
            Some('g') if modifiers.command() => Some(Message::OpenSearch(SearchMode::GoTo)),
            Some('l') if modifiers.command() => Some(Message::FindNext),
            _ => None,
        },
    }
}

/// Keys that go to the Vim engine instead of the editor's default handling.
/// Insert mode only sends Esc; arrows, Page Up/Down etc. keep their defaults.
fn vim_binding(press: &KeyPress, mode: vim::Mode, command: bool) -> Option<vim::Key> {
    let named = match press.key.as_ref() {
        Key::Named(key::Named::Escape) => Some(vim::Key::Esc),
        Key::Named(key::Named::Enter) => Some(vim::Key::Enter),
        Key::Named(key::Named::Backspace) => Some(vim::Key::Backspace),
        Key::Named(key::Named::Delete) => Some(vim::Key::Delete),
        _ => None,
    };
    if mode == vim::Mode::Insert && !command {
        return named.filter(|k| *k == vim::Key::Esc);
    }
    if named.is_some() {
        return named;
    }
    if press.modifiers.control() {
        return press.key.to_latin(press.physical_key).map(vim::Key::Ctrl);
    }
    let ch = press.text.as_deref()?.chars().find(|c| !c.is_control())?;
    Some(vim::Key::Char(ch))
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

fn dropdown(menu: Menu, vim_on: bool) -> Element<'static, Message> {
    let vim_label = if vim_on { "Vim mode           [x]" } else { "Vim mode           [ ]" };
    let options = [
        ("Dependencies...", "", Message::ShowDeps),
        (vim_label, "", Message::ToggleVim),
    ];
    let (index, items): (usize, &[(&str, &str, Message)]) = match menu {
        Menu::Edit => (1, &[("Undo", "Ctrl+Z", Message::Undo), ("Redo", "Ctrl+Y", Message::Redo)]),
        Menu::Search => (
            2,
            &[
                ("Find...", "Ctrl+F", Message::OpenSearch(SearchMode::Find)),
                ("Replace...", "Ctrl+H", Message::OpenSearch(SearchMode::Replace)),
                ("Search again", "Ctrl+L", Message::FindNext),
                ("Go to line number...", "Ctrl+G", Message::OpenSearch(SearchMode::GoTo)),
            ],
        ),
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
        Menu::Options => (6, &options),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ide(text: &str) -> Ide {
        let mut ide = Ide::default();
        ide.content = text_editor::Content::with_text(text);
        ide
    }

    fn send(ide: &mut Ide, messages: impl IntoIterator<Item = Message>) {
        for message in messages {
            let _ = ide.update(message);
        }
    }

    const TEXT: &str = "alpha beta\ngamma Alpha delta\nalpha omega\n";

    #[test]
    fn find_next_previous_and_case() {
        let mut ide = ide(TEXT);
        send(&mut ide, [
            Message::OpenSearch(SearchMode::Find),
            Message::SearchQuery("alpha".into()),
            Message::FindNext,
        ]);
        assert_eq!(ide.content.selection().as_deref(), Some("alpha"));
        assert_eq!(ide.status, "Match 1 of 3");
        let kinds: Vec<_> = ide.search_marks.iter().map(|m| (m.line, m.start, m.end, m.kind)).collect();
        assert_eq!(
            kinds,
            [(0, 0, 5, MarkKind::Current), (1, 6, 11, MarkKind::Match), (2, 0, 5, MarkKind::Match)]
        );
        send(&mut ide, [Message::FindNext]);
        assert_eq!(ide.content.selection().as_deref(), Some("Alpha"));
        assert_eq!(ide.content.cursor().position.line, 1);
        send(&mut ide, [Message::FindPrevious]);
        assert_eq!(ide.status, "Match 1 of 3");
        send(&mut ide, [Message::SearchCase(true), Message::FindNext]);
        assert_eq!(ide.content.cursor().position.line, 2, "case-sensitive skips 'Alpha'");
        assert_eq!(ide.status, "Match 2 of 2");
        send(&mut ide, [Message::FindNext]);
        assert_eq!(ide.status, "Match 1 of 2 (wrapped)");
    }

    #[test]
    fn search_again_without_the_bar() {
        let mut ide = ide(TEXT);
        send(&mut ide, [Message::FindNext]);
        assert!(ide.status.starts_with("Nothing to search for"));
        send(&mut ide, [
            Message::OpenSearch(SearchMode::Find),
            Message::SearchQuery("omega".into()),
            Message::CloseSearch,
            Message::FindNext,
        ]);
        assert_eq!(ide.content.selection().as_deref(), Some("omega"));
    }

    #[test]
    fn replace_one_all_and_undo() {
        let mut ide = ide(TEXT);
        send(&mut ide, [
            Message::OpenSearch(SearchMode::Replace),
            Message::SearchQuery("alpha".into()),
            Message::SearchReplacement("ONE".into()),
            Message::FindNext,
            Message::ReplaceOne,
        ]);
        assert_eq!(ide.content.text(), "ONE beta\ngamma Alpha delta\nalpha omega\n");
        assert_eq!(ide.content.selection().as_deref(), Some("Alpha"), "moves to the next match");

        send(&mut ide, [Message::SearchReplacement("x".into()), Message::ReplaceAll]);
        assert_eq!(ide.content.text(), "ONE beta\ngamma x delta\nx omega\n");
        assert_eq!(ide.status, "Replaced 2 occurrence(s)");
        assert!(ide.modified);

        send(&mut ide, [Message::Undo]);
        assert_eq!(ide.content.text(), "ONE beta\ngamma Alpha delta\nalpha omega\n");
        send(&mut ide, [Message::Undo]);
        assert_eq!(ide.content.text(), TEXT);
    }

    #[test]
    fn go_to_line_clamps_and_closes() {
        let mut ide = ide(TEXT);
        send(&mut ide, [
            Message::OpenSearch(SearchMode::GoTo),
            Message::SearchQuery("3".into()),
            Message::GoToLine,
        ]);
        assert_eq!(ide.content.cursor().position, Position { line: 2, column: 0 });
        assert!(ide.search.is_none());
        send(&mut ide, [
            Message::OpenSearch(SearchMode::GoTo),
            Message::SearchQuery("999".into()),
            Message::GoToLine,
        ]);
        assert_eq!(ide.content.cursor().position.line, 3, "clamped to the last line");
        send(&mut ide, [
            Message::OpenSearch(SearchMode::GoTo),
            Message::SearchQuery("abc".into()),
            Message::GoToLine,
        ]);
        assert!(ide.search.is_some(), "stays open on bad input");
        assert!(ide.status.starts_with("Enter a line number"));
    }

    #[test]
    fn find_prefills_from_selection_and_escape_closes() {
        let mut ide = ide(TEXT);
        ide.content.move_to(Cursor {
            position: Position { line: 0, column: 10 },
            selection: Some(Position { line: 0, column: 6 }),
        });
        send(&mut ide, [Message::OpenSearch(SearchMode::Find)]);
        assert_eq!(ide.search.as_ref().map(|b| b.query.as_str()), Some("beta"));
        send(&mut ide, [Message::Escape]);
        assert!(ide.search.is_none());
        assert!(ide.search_marks.is_empty(), "closing clears the highlights");
    }
}
