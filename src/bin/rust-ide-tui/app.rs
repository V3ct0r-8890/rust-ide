//! State and keyboard handling of the terminal IDE. Work that needs the
//! real terminal (building, running a program, the user screen) is returned
//! as an [`Effect`] for the main loop to carry out.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use rust_ide::runner::{self, BuildResult, Diagnostic, RunTarget};
use rust_ide::lessons::{self, Track};
use rust_ide::templates::{self, Lang, Size};
use rust_ide::{dap, filetype, lsp, platform, search, vim};

use crate::buffer::Buffer;
use crate::live::{DebugSession, OutputPane};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    File,
    Edit,
    Search,
    Run,
    Compile,
    Debug,
    Tools,
    Options,
    Window,
    Help,
}

pub const MENUS: [Menu; 10] = [
    Menu::File,
    Menu::Edit,
    Menu::Search,
    Menu::Run,
    Menu::Compile,
    Menu::Debug,
    Menu::Tools,
    Menu::Options,
    Menu::Window,
    Menu::Help,
];

impl Menu {
    pub fn title(self) -> &'static str {
        match self {
            Menu::File => "File",
            Menu::Edit => "Edit",
            Menu::Search => "Search",
            Menu::Run => "Run",
            Menu::Compile => "Compile",
            Menu::Debug => "Debug",
            Menu::Tools => "Tools",
            Menu::Options => "Options",
            Menu::Window => "Window",
            Menu::Help => "Help",
        }
    }

    fn index(self) -> usize {
        MENUS.iter().position(|&m| m == self).unwrap_or(0)
    }

    fn from_hotkey(c: char) -> Option<Menu> {
        MENUS.iter().copied().find(|m| m.title().starts_with(c.to_ascii_uppercase()))
    }
}

/// Everything a key, a menu item or a prompt can ask for.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    New,
    NewProject,
    Open,
    Save,
    SaveAs,
    Exit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    /// Shrinks runs of empty lines to one (in the selected lines, or everywhere).
    RemoveEmptyLines,
    Find,
    FindNext,
    Replace,
    /// Alt+M: the search tells upper and lower case apart.
    ToggleMatchCase,
    GoToLine,
    ToggleCheckBefore,
    ToggleVim,
    ToggleLineNumbers,
    ToggleStatusLines,
    /// Turbo, then Mono, then Modern.
    CycleTheme,
    Settings,
    Dependencies,
    GeneralEnv,
    ProjectEnv,
    ShowEnv,
    PythonSetup,
    RustToolchain,
    Run,
    /// Run in a terminal window of its own.
    RunExternal,
    /// Ctrl+F2: stop the program or the debug session.
    ProgramReset,
    /// Alt+F5: show or hide the Output window.
    UserScreen,
    ToggleBreakpoint,
    DebugRun,
    TraceInto,
    StepOver,
    StepOut,
    GoToCursor,
    AddWatch,
    ClearWatches,
    ClearBreakpoints,
    Compile,
    SyntaxCheck,
    NextMessage,
    PrevMessage,
    NextWindow,
    PrevWindow,
    CloseWindow,
    SelectWindow(usize),
    /// Alt+F6: the Markdown preview, beside the text or in a tab.
    TogglePreview,
    CyclePreviewMode,
    SetWindowStyle(WindowStyle),
    OpenUrl(&'static str),
    Help,
    /// Help > Learn: the built-in lessons of one language.
    Learn(Track),
    /// The line between groups of a menu: does nothing.
    Separator,
}

pub struct MenuItem {
    pub label: String,
    pub hotkey: char,
    pub keys: &'static str,
    pub action: Action,
}

fn item(label: impl Into<String>, hotkey: char, keys: &'static str, action: Action) -> MenuItem {
    MenuItem { label: label.into(), hotkey, keys, action }
}

/// A line between groups of menu items.
fn separator() -> MenuItem {
    item("", '\0', "", Action::Separator)
}

impl MenuItem {
    pub fn is_separator(&self) -> bool {
        self.action == Action::Separator
    }
}

/// The next item that is not a separator, going down (or up), wrapping around.
fn next_item(items: &[MenuItem], from: usize, down: bool) -> usize {
    let len = items.len();
    let mut at = from;
    for _ in 0..len {
        at = if down { (at + 1) % len } else { (at + len - 1) % len };
        if !items[at].is_separator() {
            return at;
        }
    }
    from
}

/// The right-click menu of the editor.
pub fn context_items() -> Vec<MenuItem> {
    vec![
        item("Cut", 't', "Ctrl+X", Action::Cut),
        item("Copy", 'c', "Ctrl+C", Action::Copy),
        item("Paste", 'p', "Ctrl+V", Action::Paste),
        item("Select all", 'a', "Ctrl+A", Action::SelectAll),
        separator(),
        item("Remove double empty lines", 'd', "", Action::RemoveEmptyLines),
    ]
}

/// Rows of Options > Settings (the window version's, without the font size).
pub const SETTINGS_ROWS: [(&str, char); 9] = [
    ("Markdown preview opens", 'p'),
    ("Window style", 'w'),
    ("Vim mode", 'v'),
    ("Check before run", 'c'),
    ("Line numbers", 'n'),
    ("Theme", 't'),
    ("Two-line status", 's'),
    ("Dependencies...", 'd'),
    ("Close", '\0'),
];

/// How open files (windows) are shown and chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowStyle {
    /// Turbo Pascal: a title line with the name and the window number.
    Legacy,
    /// A row of tabs under the menu bar.
    #[default]
    Horizontal,
    /// A list of the files on the left of the editor.
    Vertical,
}

impl WindowStyle {
    pub const ALL: [WindowStyle; 3] = [WindowStyle::Legacy, WindowStyle::Horizontal, WindowStyle::Vertical];

    /// The value kept in the settings file (shared with the window version).
    pub fn setting(self) -> &'static str {
        match self {
            WindowStyle::Legacy => "legacy",
            WindowStyle::Horizontal => "horizontal",
            WindowStyle::Vertical => "vertical",
        }
    }

    pub fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("legacy") => WindowStyle::Legacy,
            Some("vertical") => WindowStyle::Vertical,
            _ => WindowStyle::Horizontal,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            WindowStyle::Legacy => "Legacy windows",
            WindowStyle::Horizontal => "Horizontal tabs",
            WindowStyle::Vertical => "Vertical tabs",
        }
    }
}

/// Where the Markdown preview is shown. (The window version can also use a separate
/// window; a terminal has only one, so that setting shows it in a tab here.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewMode {
    #[default]
    Side,
    Tab,
}

impl PreviewMode {
    pub fn setting(self) -> &'static str {
        match self {
            PreviewMode::Side => "side",
            PreviewMode::Tab => "tab",
        }
    }

    pub fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("tab" | "window") => PreviewMode::Tab,
            _ => PreviewMode::Side,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PreviewMode::Side => "side by side",
            PreviewMode::Tab => "a new tab",
        }
    }
}

/// What happens after a successful build.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum After {
    Compile,
    Run,
    /// Run in a terminal window of its own.
    External,
    Debug(dap::Start),
}

/// The Find / Replace bar above the editor; while it is open, keys go to it.
#[derive(Debug, Clone, Default)]
pub struct SearchBar {
    pub replace: bool,
    pub query: String,
    pub replacement: String,
    /// 0: the Find field, 1: the Replace field.
    pub field: usize,
}

/// "Turbo", "Mono" or "Modern".
pub fn theme_label() -> &'static str {
    match rust_ide::platform::theme_setting() {
        "mono" => "Mono",
        "modern" => "Modern",
        _ => "Turbo",
    }
}

/// Switches to a theme by its setting name (turbo, mono, modern).
pub fn set_theme(name: &str) {
    match name {
        "mono" => rust_ide::platform::set_mono(true),
        "modern" => rust_ide::platform::set_modern(true),
        _ => rust_ide::platform::set_mono(false),
    }
}

/// The completion list under the cursor: words of the file, keywords and the language server's
/// suggestions. Only the items that start like the typed word are shown.
pub struct Completion {
    pub items: Vec<rust_ide::lsp::CompletionItem>,
    pub selected: usize,
    /// Byte column where the word being completed starts (on the cursor's line).
    pub start: usize,
    /// The language server request this list answers (0: no request).
    pub id: u64,
    /// Waiting for the language server.
    pub waiting: bool,
}

/// Tools > Dependencies: the tools the IDE uses, found or missing.
pub struct DepsDialog {
    pub list: Vec<rust_ide::deps::Dep>,
    pub selected: usize,
    pub message: String,
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// What the main loop must do with the terminal.
#[derive(Debug, PartialEq)]
pub enum Effect {
    None,
    /// Build, then do `after`; the status says "Compiling...".
    Build { path: PathBuf, after: After },
    /// Look for errors only: no program is built or run.
    Check { path: PathBuf },
    /// Run a built program with its output in the Output window (events come back as
    /// `Background::Output(run_id, ..)`).
    StartRun { target: RunTarget, run_id: u64 },
    /// Start the debugger (events come back as `Background::Debug(run_id, ..)`).
    StartDebug { launch: dap::Launch, run_id: u64 },
    /// Run a program on the terminal's own screen until it ends (when no terminal window
    /// of its own can be opened).
    RunHere(RunTarget),
    /// Create the project's .venv and install its requirements (slow).
    PythonSetup(PathBuf),
    /// Put the text on the system clipboard.
    Copy(String),
    /// Read the system clipboard and call `App::paste`.
    Paste,
    /// Look for the tools (slow) and call `App::deps_checked` (`startup`: only show the
    /// dialog when something is missing).
    CheckDeps { startup: bool },
    /// Run a dependency's install command and call `App::dep_installed`.
    InstallDep { name: &'static str, command: Vec<String> },
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    SaveAs,
    GoTo,
    /// Options > Rust toolchain: stable, nightly, 1.85.0 ... (empty = default).
    Toolchain,
    /// Debug > Add watch: an expression evaluated at each stop.
    Watch,
}

pub enum Mode {
    Edit,
    Menu { menu: Menu, item: usize },
    Prompt { kind: PromptKind, input: String },
    Open(OpenDialog),
    Help { scroll: usize },
    /// "Choose a language" list (start-up and File > New).
    Language { selected: usize, startup: bool },
    /// File > New project: language, size, name and folder.
    Project(ProjectDialog),
    /// Options > Rust toolchain: the installed toolchains to pick from (the row
    /// after the last one asks for a name).
    Toolchains { choices: Vec<rust_ide::env::ToolchainChoice>, selected: usize },
    /// Close or exit with unsaved changes: Save / Discard / Cancel (`button` is the one Tab is on).
    Unsaved { action: Action, button: usize },
    /// Help > Learn: the lessons of one language to pick from.
    Lessons { track: Track, selected: usize },
    /// The right-click menu, its top left corner at `x`, `y`.
    Context { x: u16, y: u16, item: usize },
    /// Options > Settings: the main options in one list.
    Settings { selected: usize },
    Deps(DepsDialog),
}

/// Lessons shown at once in the list.
pub const LESSON_ROWS: usize = 12;

pub const PROJECT_LANG: usize = 0;
pub const PROJECT_SIZE: usize = 1;
pub const PROJECT_NAME: usize = 2;
pub const PROJECT_FOLDER: usize = 3;
pub const PROJECT_CREATE: usize = 4;
pub const PROJECT_ROWS: usize = 6;

pub struct ProjectDialog {
    pub lang: Lang,
    pub size: Size,
    pub name: String,
    pub folder: String,
    /// The focused row (`PROJECT_*`).
    pub row: usize,
    /// Why the last Create failed.
    pub error: String,
}

/// F3: a Turbo-style file list of one directory.
pub struct OpenDialog {
    pub dir: PathBuf,
    pub entries: Vec<(String, bool)>,
    pub selected: usize,
    pub filter: String,
}

impl OpenDialog {
    fn new(dir: PathBuf) -> Self {
        let mut dialog = Self { dir, entries: Vec::new(), selected: 0, filter: String::new() };
        dialog.read();
        dialog
    }

    fn read(&mut self) {
        let mut entries: Vec<(String, bool)> = std::fs::read_dir(&self.dir)
            .map(|list| {
                list.flatten()
                    .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path().is_dir()))
                    .filter(|(name, _)| !name.starts_with('.'))
                    .collect()
            })
            .unwrap_or_default();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
        if self.dir.parent().is_some() {
            entries.insert(0, ("..".into(), true));
        }
        self.entries = entries;
        self.selected = 0;
    }

    /// Entries matching the typed filter.
    pub fn visible(&self) -> Vec<&(String, bool)> {
        let filter = self.filter.to_lowercase();
        self.entries.iter().filter(|(name, _)| name.to_lowercase().starts_with(&filter)).collect()
    }
}

pub struct App {
    pub windows: Vec<Buffer>,
    pub active: usize,
    pub mode: Mode,
    pub status: String,
    /// Results of the last build.
    pub messages: Vec<Diagnostic>,
    pub message_index: Option<usize>,
    pub last_query: String,
    /// Options > Check before run: F5 / F9 look for errors first.
    pub check_before: bool,
    /// Options > Line numbers: a column of line numbers left of the text.
    pub line_numbers: bool,
    /// Remember the folder of opened files (off in tests, so they leave the settings alone).
    pub remember_dir: bool,
    /// After a check that finds no errors: build, then this.
    then_build: Option<After>,
    /// Program shown on the user screen (for its title).
    pub last_run: Option<String>,
    /// Options > Two-line status: messages get their own line above the keys.
    pub two_line_status: bool,
    pub completion: Option<Completion>,
    pub next_completion_id: u64,
    /// The last text cut or copied (used when the system clipboard cannot be read).
    pub clipboard: String,
    /// Where a mouse drag started (row, character column).
    drag_from: Option<(usize, usize)>,
    pub search: Option<SearchBar>,
    /// The Replace field, kept while the bar is closed.
    last_replacement: String,
    /// Match case (Alt+M), for the bar and Search again.
    pub match_case: bool,
    pub output: OutputPane,
    pub debug: Option<DebugSession>,
    pub debug_run_id: u64,
    /// Breakpoints: file -> 0-based lines.
    pub breakpoints: BTreeMap<PathBuf, BTreeSet<usize>>,
    /// Watch expressions and their last values.
    pub watches: Vec<(String, String)>,
    /// The language server client, once it is ready.
    pub lsp: Option<iced::futures::channel::mpsc::UnboundedSender<lsp::Request>>,
    /// The file the language server follows, and its window's id.
    pub lsp_doc: Option<PathBuf>,
    pub lsp_id: u64,
    /// (window id, text version) last seen by `sync`.
    pub synced: (u64, u64),
    /// The language server's errors and warnings for the shown file.
    pub live: Vec<lsp::Diagnostic>,
    pub lsp_status: String,
    pub vim: Option<vim::Vim>,
    /// Markdown preview beside the text (in Side mode).
    pub preview: bool,
    pub preview_mode: PreviewMode,
    pub window_style: WindowStyle,
    /// Tools > Dependencies, "Check at startup".
    pub check_at_startup: bool,
}

impl App {
    pub fn new(files: &[PathBuf]) -> Self {
        let mut app = Self {
            windows: vec![Buffer::default()],
            active: 0,
            mode: Mode::Edit,
            status: String::new(),
            messages: Vec::new(),
            message_index: None,
            last_query: String::new(),
            check_before: false,
            line_numbers: false,
            remember_dir: false,
            then_build: None,
            last_run: None,
            two_line_status: false,
            completion: None,
            next_completion_id: 0,
            clipboard: String::new(),
            drag_from: None,
            search: None,
            last_replacement: String::new(),
            match_case: false,
            output: OutputPane::default(),
            debug: None,
            debug_run_id: 0,
            breakpoints: BTreeMap::new(),
            watches: Vec::new(),
            lsp: None,
            lsp_doc: None,
            lsp_id: 0,
            synced: (0, 0),
            live: Vec::new(),
            lsp_status: String::new(),
            vim: None,
            preview: false,
            preview_mode: PreviewMode::Side,
            window_style: WindowStyle::Horizontal,
            check_at_startup: true,
        };
        for file in files {
            if file.exists() {
                app.open(file);
            } else {
                app.new_window();
                app.buffer_mut().path = Some(file.clone());
                app.status = format!("New file {}", file.display());
            }
        }
        app.active = 0;
        app
    }

    // ----- Sessions -----

    /// The open files with their cursors, in window order.
    pub fn session_snapshot(&self) -> rust_ide::session::Session {
        let mut session = rust_ide::session::Session::default();
        for (index, window) in self.windows.iter().enumerate() {
            if let Some(path) = &window.path {
                if index == self.active {
                    session.active = session.files.len();
                }
                session.files.push(rust_ide::session::Entry { path: path.clone(), line: window.row, column: window.char_col() });
            }
        }
        session
    }

    /// Remembers the open files for the next start.
    pub fn save_session(&self) {
        let session = self.session_snapshot();
        if session.files.is_empty() {
            if let Ok(cwd) = std::env::current_dir() {
                rust_ide::session::forget(&rust_ide::env::project_root(&cwd));
            }
        } else {
            let _ = rust_ide::session::save(&session);
        }
    }

    /// Opens the files of an earlier session, each with its cursor, on the window that was active.
    pub fn restore_session(&mut self, session: &rust_ide::session::Session) {
        let mut opened = 0;
        for entry in &session.files {
            if entry.path.is_file() {
                self.open(&entry.path);
                self.buffer_mut().go_to(entry.line, entry.column);
                opened += 1;
            }
        }
        if let Some(index) = session.files.get(session.active).and_then(|e| self.windows.iter().position(|w| w.path.as_deref() == Some(e.path.as_path()))) {
            self.select(index);
        }
        self.status = format!("Restored the last session: {opened} file(s)");
    }

    pub fn buffer(&self) -> &Buffer {
        &self.windows[self.active]
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.windows[self.active]
    }

    pub fn menu_items(&self, menu: Menu) -> Vec<MenuItem> {
        match menu {
            Menu::File => vec![
                item("New", 'n', "", Action::New),
                item("New project...", 'p', "", Action::NewProject),
                item("Open...", 'o', "F3", Action::Open),
                item("Save", 's', "F2", Action::Save),
                item("Save as...", 'a', "", Action::SaveAs),
                item("Exit", 'x', "Alt+X", Action::Exit),
            ],
            Menu::Edit => vec![
                item("Undo", 'u', "Ctrl+Z", Action::Undo),
                item("Redo", 'r', "Ctrl+Y", Action::Redo),
                separator(),
                item("Cut", 't', "Ctrl+X", Action::Cut),
                item("Copy", 'c', "Ctrl+C", Action::Copy),
                item("Paste", 'p', "Ctrl+V", Action::Paste),
                item("Select all", 'a', "Ctrl+A", Action::SelectAll),
                separator(),
                item("Remove double empty lines", 'd', "", Action::RemoveEmptyLines),
            ],
            Menu::Search => vec![
                item("Find...", 'f', "Ctrl+F", Action::Find),
                item("Replace...", 'r', "Ctrl+H", Action::Replace),
                item("Search again", 's', "Ctrl+L", Action::FindNext),
                item("Go to line number...", 'g', "Ctrl+G", Action::GoToLine),
            ],
            Menu::Run => vec![
                item("Run", 'r', "F5", Action::Run),
                item("Run in external terminal", 'e', "", Action::RunExternal),
                item("Program reset", 'p', "Ctrl+F2", Action::ProgramReset),
                item("User screen", 'u', "Alt+F5", Action::UserScreen),
            ],
            Menu::Compile => vec![
                item("Compile", 'c', "F9", Action::Compile),
                item("Syntax check", 's', "Alt+F9", Action::SyntaxCheck),
                item("Next message", 'n', "Alt+F8", Action::NextMessage),
                item("Previous message", 'p', "Alt+F7", Action::PrevMessage),
            ],
            Menu::Debug => vec![
                item("Toggle breakpoint", 'b', "Ctrl+F8", Action::ToggleBreakpoint),
                item("Debug run", 'd', "Shift+F5", Action::DebugRun),
                item("Trace into", 't', "F7", Action::TraceInto),
                item("Step over", 's', "F8", Action::StepOver),
                item("Step out", 'o', "Shift+F8", Action::StepOut),
                item("Go to cursor", 'g', "F4", Action::GoToCursor),
                item("Add watch...", 'w', "Ctrl+F7", Action::AddWatch),
                item("Clear watches", 'c', "", Action::ClearWatches),
                item("Clear breakpoints", 'l', "", Action::ClearBreakpoints),
                item("Program reset", 'r', "Ctrl+F2", Action::ProgramReset),
            ],
            Menu::Tools => vec![
                item("Dependencies...", 'd', "", Action::Dependencies),
                item("Rust toolchain...", 'r', "", Action::RustToolchain),
                item("Python .venv setup", 'v', "", Action::PythonSetup),
                separator(),
                item("General environment...", 'g', "", Action::GeneralEnv),
                item("Project environment...", 'p', "", Action::ProjectEnv),
                item("Show environment", 's', "", Action::ShowEnv),
            ],
            Menu::Options => {
                let on = |flag: bool, label: &str| format!("{label:<18}{}", if flag { "[x]" } else { "[ ]" });
                vec![
                    item("Settings...", 's', "", Action::Settings),
                    separator(),
                    item(on(self.vim.is_some(), "Vim mode"), 'v', "", Action::ToggleVim),
                    item(on(self.check_before, "Check before run"), 'c', "", Action::ToggleCheckBefore),
                    item(on(self.line_numbers, "Line numbers"), 'n', "", Action::ToggleLineNumbers),
                    item(on(self.two_line_status, "Two-line status"), 'w', "", Action::ToggleStatusLines),
                    separator(),
                    item(format!("Theme: {}", theme_label()), 't', "", Action::CycleTheme),
                ]
            }
            Menu::Window => {
                const ALT_KEYS: [&str; 9] = ["Alt+1", "Alt+2", "Alt+3", "Alt+4", "Alt+5", "Alt+6", "Alt+7", "Alt+8", "Alt+9"];
                let mut items = vec![
                    item("Next", 'n', "F6", Action::NextWindow),
                    item("Previous", 'p', "Shift+F6", Action::PrevWindow),
                    item("Close", 'c', "Ctrl+F3", Action::CloseWindow),
                    item(
                        if self.preview_shown() { "Markdown preview  [x]" } else { "Markdown preview  [ ]" },
                        'm',
                        "Alt+F6",
                        Action::TogglePreview,
                    ),
                    item(format!("Preview in: {}", self.preview_mode.label()), 'i', "", Action::CyclePreviewMode),
                ];
                // How windows are shown: pick one.
                for (style, hotkey) in WindowStyle::ALL.into_iter().zip(['l', 'h', 't']) {
                    let mark = if self.window_style == style { "(*)" } else { "( )" };
                    items.push(item(format!("{:<19}{mark}", style.label()), hotkey, "", Action::SetWindowStyle(style)));
                }
                for (index, window) in self.windows.iter().enumerate() {
                    let mark = if index == self.active { '>' } else { ' ' };
                    let star = if window.modified { "*" } else { "" };
                    let hotkey = char::from_digit((index as u32 + 1) % 10, 10).unwrap_or(' ');
                    let keys = ALT_KEYS.get(index).copied().unwrap_or("");
                    items.push(item(format!("{}{mark}{}{star}", index + 1, window.name()), hotkey, keys, Action::SelectWindow(index)));
                }
                items
            }
            Menu::Help => vec![
                item("Keyboard shortcuts", 'k', "F1", Action::Help),
                item("Learn Rust...", 'r', "", Action::Learn(Track::Rust)),
                item("Learn Python...", 'y', "", Action::Learn(Track::Python)),
                item("Learn C#...", 'c', "", Action::Learn(Track::CSharp)),
                item("Install guide (GitHub)", 'i', "", Action::OpenUrl(platform::INSTALL_GUIDE_URL)),
                item("Project page (GitHub)", 'p', "", Action::OpenUrl(platform::GITHUB_URL)),
            ],
        }
    }

    // ----- Keys -----

    pub fn key(&mut self, key: KeyEvent) -> Effect {
        // Global keys work in every mode (Ctrl+R is Vim's redo while Vim is on).
        let vim_redo = self.vim.as_ref().is_some_and(|v| v.mode != vim::Mode::Insert)
            && key.code == KeyCode::Char('r')
            && key.modifiers == KeyModifiers::CONTROL
            && matches!(self.mode, Mode::Edit)
            && self.search.is_none()
            && !self.output.visible;
        if let Some(action) = global_key(&key).filter(|_| !vim_redo) {
            // The unsaved-changes dialog is about one window: keys that switch windows wait.
            if matches!(self.mode, Mode::Unsaved { .. })
                && matches!(action, Action::NextWindow | Action::PrevWindow | Action::SelectWindow(_) | Action::New | Action::Open)
            {
                return Effect::None;
            }
            self.mode = Mode::Edit;
            self.completion = None;
            return self.act(action);
        }
        let on_preview = self.buffer().preview_of.is_some();
        match &mut self.mode {
            Mode::Edit if self.output.visible => self.output_key(key),
            Mode::Edit if self.search.is_some() => self.search_key(key),
            Mode::Edit if on_preview => self.preview_key(key),
            Mode::Edit => self.edit_key(key),
            Mode::Menu { .. } => self.menu_key(key),
            Mode::Prompt { .. } => self.prompt_key(key),
            Mode::Open(_) => self.open_key(key),
            Mode::Language { .. } => self.language_key(key),
            Mode::Project(_) => self.project_key(key),
            Mode::Toolchains { .. } => self.toolchain_key(key),
            Mode::Unsaved { .. } => self.unsaved_key(key),
            Mode::Lessons { .. } => self.lessons_key(key),
            Mode::Context { .. } => self.context_key(key),
            Mode::Settings { .. } => self.settings_key(key),
            Mode::Deps(_) => self.deps_key(key),
            Mode::Help { scroll } => {
                match key.code {
                    KeyCode::Up => *scroll = scroll.saturating_sub(1),
                    KeyCode::Down => *scroll += 1,
                    KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                    KeyCode::PageDown => *scroll += 10,
                    KeyCode::Esc | KeyCode::Enter => self.mode = Mode::Edit,
                    _ => {}
                }
                Effect::None
            }
        }
    }

    fn edit_key(&mut self, key: KeyEvent) -> Effect {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        if self.completion.is_some() && self.completion_key(key.code) {
            return Effect::None;
        }
        // Ctrl+Space (some terminals send it as a NUL) asks for completion.
        if key.code == KeyCode::Null || ctrl && !alt && key.code == KeyCode::Char(' ') {
            self.request_completion(None);
            return Effect::None;
        }
        if let Some(vim) = &self.vim
            && self.completion.is_none()
            && let Some(vim_key) = crate::vimkeys::binding(&key, vim)
        {
            return self.vim_key(vim_key);
        }
        let typed = match key.code {
            KeyCode::Char(c) if !ctrl && !alt || ctrl && alt => Some(c),
            _ => None,
        };
        // Typing a word (or Backspace) keeps the list open and narrows it.
        let keeps_list = typed.is_some_and(is_ident) || key.code == KeyCode::Backspace;
        if !keeps_list {
            self.completion = None;
        }
        let moving = matches!(
            key.code,
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End
        );
        let b = self.buffer_mut();
        if moving {
            if shift { b.start_selection() } else { b.clear_selection() }
        }
        match key.code {
            KeyCode::Char(c) if !ctrl && !alt || ctrl && alt => {
                b.delete_selection();
                b.insert_char(c);
            }
            KeyCode::Enter => {
                b.delete_selection();
                b.newline();
            }
            KeyCode::Backspace => {
                if !b.delete_selection() {
                    b.backspace();
                }
            }
            KeyCode::Delete => {
                if !b.delete_selection() {
                    b.delete();
                }
            }
            KeyCode::Tab => {
                b.clear_selection();
                b.indent();
            }
            KeyCode::BackTab => {
                b.clear_selection();
                b.unindent();
            }
            KeyCode::Left if ctrl => b.word_left(),
            KeyCode::Right if ctrl => b.word_right(),
            KeyCode::Left => b.left(),
            KeyCode::Right => b.right(),
            KeyCode::Up => b.vertical(-1),
            KeyCode::Down => b.vertical(1),
            KeyCode::PageUp => b.vertical(-20),
            KeyCode::PageDown => b.vertical(20),
            KeyCode::Home if ctrl => b.document_start(),
            KeyCode::End if ctrl => b.document_end(),
            KeyCode::Home => b.home(),
            KeyCode::End => b.end(),
            _ => {}
        }
        if self.completion.is_some() {
            self.refresh_completion();
        } else if typed.is_some_and(is_ident) {
            self.complete(false);
        }
        // `.` and `::` ask the language server what can follow.
        if let Some(c) = typed
            && self.lsp_doc.is_some()
        {
            let b = self.buffer();
            let before = b.lines[b.row][..b.col].chars().rev().nth(1);
            if c == '.' || (c == ':' && b.lang() == rust_ide::highlight::Lang::Rust && before == Some(':')) {
                self.request_completion(Some(c));
            }
        }
        Effect::None
    }

    /// A key for the Vim engine (Normal, Visual and command-line keys, and Esc).
    fn vim_key(&mut self, key: vim::Key) -> Effect {
        let Some(vim) = &mut self.vim else { return Effect::None };
        let effects = crate::vimkeys::key(vim, key, &mut self.windows[self.active]);
        for effect in effects {
            match effect {
                vim::Effect::Edited => {}
                vim::Effect::Undo => {
                    self.act(Action::Undo);
                }
                vim::Effect::Redo => {
                    self.act(Action::Redo);
                }
                vim::Effect::Status(status) => self.status = status,
                vim::Effect::Save => self.save(),
                vim::Effect::Quit if self.buffer().modified => {
                    self.status = "No write since last change (add ! to override)".into();
                }
                vim::Effect::Quit => return self.act(Action::Exit),
                vim::Effect::ForceQuit => return Effect::Quit,
                vim::Effect::SaveQuit => {
                    if self.buffer().path.is_none() {
                        self.status = "No file name (use F2 to save first)".into();
                    } else {
                        self.save();
                        if !self.buffer().modified {
                            return self.act(Action::Exit);
                        }
                    }
                }
            }
        }
        Effect::None
    }

    // ----- Search bar -----

    fn search_key(&mut self, key: KeyEvent) -> Effect {
        let Some(bar) = &mut self.search else { return Effect::None };
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Esc => {
                self.last_replacement = std::mem::take(&mut bar.replacement);
                self.search = None;
            }
            KeyCode::Char('m' | 'M') if alt => return self.act(Action::ToggleMatchCase),
            KeyCode::Char('a' | 'A') if alt && bar.replace => self.replace_all(),
            KeyCode::Tab | KeyCode::BackTab if bar.replace => bar.field = 1 - bar.field,
            KeyCode::Enter if bar.field == 1 => self.replace_one(),
            KeyCode::Enter | KeyCode::Down => self.find(true),
            KeyCode::Up => self.find(false),
            KeyCode::Backspace => {
                if bar.field == 0 { bar.query.pop() } else { bar.replacement.pop() };
            }
            KeyCode::Char(c) if !alt => {
                if bar.field == 0 { bar.query.push(c) } else { bar.replacement.push(c) }
            }
            _ => {}
        }
        Effect::None
    }

    /// The text to look for: the bar's, else the last one.
    fn search_query(&mut self) -> Option<String> {
        if let Some(bar) = &self.search {
            self.last_query = bar.query.clone();
        }
        if self.last_query.is_empty() {
            self.status = "Nothing to search for (Ctrl+F)".into();
            return None;
        }
        Some(self.last_query.clone())
    }

    /// Selects the next (or previous) match.
    pub fn find(&mut self, forward: bool) {
        let Some(query) = self.search_query() else { return };
        let case = self.match_case;
        let text = self.buffer().text();
        let here = self.buffer().offset();
        let other = self.buffer().anchor.map_or(here, |(row, col)| self.buffer().offset_of(row, col));
        let found = if forward {
            search::find(&text, &query, here.max(other), case)
        } else {
            search::find_back(&text, &query, here.min(other), case)
        };
        let Some((start, end, wrapped)) = found else {
            self.status = format!("Search string not found: {query}");
            return;
        };
        let b = self.buffer_mut();
        b.go_to_offset(start);
        b.start_selection();
        b.go_to_offset_keeping(end);
        let matches = search::all(&text, &query, case);
        let index = matches.iter().position(|&(s, _)| s == start).map_or(0, |i| i + 1);
        let wrapped = if wrapped { " (wrapped)" } else { "" };
        self.status = format!("Match {index} of {}{wrapped}", matches.len());
    }

    fn replace_one(&mut self) {
        let Some(query) = self.search_query() else { return };
        let replacement = self.search.as_ref().map(|b| b.replacement.clone()).unwrap_or_default();
        let selected = self.buffer().selected_text().unwrap_or_default();
        if search::is_match(&selected, &query, self.match_case) {
            self.buffer_mut().replace_selection(&replacement);
        }
        self.find(true);
    }

    fn replace_all(&mut self) {
        let Some(query) = self.search_query() else { return };
        let replacement = self.search.as_ref().map(|b| b.replacement.clone()).unwrap_or_default();
        let case = self.match_case;
        let count = self.buffer_mut().replace_all(&query, &replacement, case);
        self.status =
            if count == 0 { format!("Search string not found: {query}") } else { format!("Replaced {count} occurrence(s)") };
    }

    // ----- Markdown preview -----

    /// A Markdown file: by its name, or the chosen type of an untitled window.
    pub fn is_markdown(&self) -> bool {
        let b = self.buffer();
        let ext = match &b.path {
            Some(path) => path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase),
            None => b.ext.map(str::to_string),
        };
        b.preview_of.is_none() && matches!(ext.as_deref(), Some("md" | "markdown"))
    }

    /// The preview is drawn next to the editor.
    pub fn preview_visible(&self) -> bool {
        self.preview_mode == PreviewMode::Side && self.preview && self.is_markdown()
    }

    /// For the [x] / [ ] of the Window menu.
    fn preview_shown(&self) -> bool {
        match self.preview_mode {
            PreviewMode::Side => self.preview,
            PreviewMode::Tab => {
                let id = self.buffer().id;
                self.buffer().preview_of.is_some() || self.windows.iter().any(|w| w.preview_of == Some(id))
            }
        }
    }

    /// The text shown by the active preview tab (empty when its file was closed).
    pub fn preview_source(&self) -> Option<&Buffer> {
        let id = self.buffer().preview_of?;
        self.windows.iter().find(|w| w.id == id)
    }

    fn toggle_preview(&mut self) {
        match self.preview_mode {
            PreviewMode::Side => {
                if !self.is_markdown() {
                    self.status = "The preview is for Markdown (.md) files".into();
                } else {
                    self.preview = !self.preview;
                    self.status = format!("Markdown preview {}", if self.preview { "on" } else { "off" });
                }
            }
            PreviewMode::Tab => {
                if let Some(source) = self.buffer().preview_of {
                    // On the preview tab: close it and go back to the file.
                    self.close_window();
                    if let Some(index) = self.windows.iter().position(|w| w.id == source) {
                        self.active = index;
                    }
                    self.status = "Markdown preview tab closed".into();
                } else if !self.is_markdown() {
                    self.status = "The preview is for Markdown (.md) files".into();
                } else {
                    let id = self.buffer().id;
                    match self.windows.iter().position(|w| w.preview_of == Some(id)) {
                        Some(index) => self.active = index,
                        None => {
                            let title = format!("Preview {}", self.buffer().name());
                            self.windows.insert(self.active + 1, Buffer::preview(id, title));
                            self.active += 1;
                        }
                    }
                    self.status = "Markdown preview tab (Alt+F6 closes it)".into();
                }
            }
        }
    }

    fn set_preview_mode(&mut self, mode: PreviewMode) {
        self.preview_mode = mode;
        self.preview = false;
        self.status = format!("Markdown preview opens {}", mode.label());
        let _ = platform::write_setting("preview_mode", mode.setting());
    }

    /// Keys on a preview tab: it scrolls, and cannot be edited.
    fn preview_key(&mut self, key: KeyEvent) -> Effect {
        let b = self.buffer_mut();
        match key.code {
            KeyCode::Up => b.scroll = b.scroll.saturating_sub(1),
            KeyCode::Down => b.scroll += 1,
            KeyCode::PageUp => b.scroll = b.scroll.saturating_sub(20),
            KeyCode::PageDown => b.scroll += 20,
            KeyCode::Home => b.scroll = 0,
            KeyCode::Esc => {}
            _ => self.status = "The preview cannot be edited (Alt+F6 closes it)".into(),
        }
        Effect::None
    }

    // ----- Word completion -----

    /// Keys of the open completion list; false when the key is for the editor.
    fn completion_key(&mut self, code: KeyCode) -> bool {
        let count = self.visible_completions().len();
        let Some(list) = &mut self.completion else { return false };
        if code == KeyCode::Esc {
            self.completion = None;
            return true;
        }
        if count == 0 {
            return false; // still waiting for the language server
        }
        match code {
            KeyCode::Up => list.selected = (list.selected + count - 1) % count,
            KeyCode::Down => list.selected = (list.selected + 1) % count,
            KeyCode::PageUp => list.selected = list.selected.saturating_sub(8),
            KeyCode::PageDown => list.selected = (list.selected + 8).min(count - 1),
            KeyCode::Enter | KeyCode::Tab => {
                let selected = list.selected;
                let start = list.start;
                let word = self.visible_completions()[selected].insert.clone();
                self.completion = None;
                self.buffer_mut().replace_before_cursor(start, &word);
            }
            _ => return false,
        }
        true
    }

    /// The typed part of the word being completed, or None if the cursor left it.
    pub fn completion_prefix(&self) -> Option<String> {
        let start = self.completion.as_ref()?.start;
        let b = self.buffer();
        let prefix = b.lines[b.row].get(start..b.col)?;
        prefix.chars().all(is_ident).then(|| prefix.to_string())
    }

    /// The items that start like the typed word.
    pub fn visible_completions(&self) -> Vec<&rust_ide::lsp::CompletionItem> {
        let (Some(completion), Some(prefix)) = (&self.completion, self.completion_prefix()) else {
            return Vec::new();
        };
        let prefix = prefix.to_lowercase();
        completion.items.iter().filter(|item| item.filter.to_lowercase().starts_with(&prefix)).collect()
    }

    /// After typing: narrows the list, or closes it when nothing fits.
    pub fn refresh_completion(&mut self) {
        let waiting = self.completion.as_ref().is_some_and(|c| c.waiting);
        let local = self.completion.as_ref().is_some_and(|c| c.id == 0);
        let visible = self.visible_completions().len();
        let short = self.completion_prefix().is_none_or(|p| local && p.chars().count() < 2);
        if short || (!waiting && visible == 0) {
            self.completion = None;
            // Keep offering the words of the file while one is typed.
            if self.buffer().col > 0 {
                self.complete(false);
            }
        } else if let Some(completion) = &mut self.completion {
            completion.selected = completion.selected.min(visible.saturating_sub(1));
        }
    }

    /// Opens the list of words of the file and keywords that start like the word before the
    /// cursor: after two letters while typing, or after one when asked (Ctrl+Space).
    pub fn complete(&mut self, asked: bool) {
        self.completion = None;
        let b = self.buffer();
        let lang = b.lang();
        if lang == rust_ide::highlight::Lang::Plain && !asked {
            return;
        }
        let before = &b.lines[b.row][..b.col];
        let start = before.trim_end_matches(is_ident).len();
        let prefix = before[start..].to_string();
        if prefix.chars().count() < if asked { 1 } else { 2 } || prefix.starts_with(|c: char| c.is_ascii_digit()) {
            if asked {
                self.status = "Type the start of a word first".into();
            }
            return;
        }
        let items = rust_ide::text::local_items(&b.text(), rust_ide::highlight::keywords(lang), &prefix);
        if items.is_empty() {
            if asked {
                self.status = format!("No words start with \"{prefix}\"");
            }
            return;
        }
        self.completion = Some(Completion { items, selected: 0, start, id: 0, waiting: false });
    }

    // ----- Clipboard -----

    /// Inserts pasted text in place of the selection.
    pub fn paste(&mut self, text: &str) {
        if text.is_empty() {
            self.status = "The clipboard is empty".into();
            return;
        }
        self.buffer_mut().insert_text(text);
        self.status = format!("Pasted {} line(s)", text.lines().count().max(1));
    }

    /// Text pasted by the terminal itself (bracketed paste).
    pub fn paste_event(&mut self, text: &str) {
        let first_line = text.lines().next().unwrap_or("");
        match &mut self.mode {
            Mode::Edit if self.output.visible => self.output.input.push_str(first_line),
            Mode::Edit if self.search.is_some() => {
                if let Some(bar) = &mut self.search {
                    if bar.field == 0 { bar.query.push_str(first_line) } else { bar.replacement.push_str(first_line) }
                }
            }
            Mode::Edit => self.paste(text),
            Mode::Prompt { input, .. } => input.push_str(first_line),
            _ => {}
        }
    }

    fn copy_selection(&mut self, cut: bool) -> Effect {
        let Some(text) = self.buffer().selected_text() else {
            self.status = "Select text first (Shift+arrows, or drag with the mouse)".into();
            return Effect::None;
        };
        if cut {
            self.buffer_mut().delete_selection();
        }
        self.status = format!("{} {} character(s)", if cut { "Cut" } else { "Copied" }, text.chars().count());
        self.clipboard = text.clone();
        Effect::Copy(text)
    }

    // ----- Right-click menu, Settings, Dependencies -----

    fn context_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Context { x, y, item } = self.mode else { return Effect::None };
        let items = context_items();
        match key.code {
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Up => self.mode = Mode::Context { x, y, item: next_item(&items, item, false) },
            KeyCode::Down => self.mode = Mode::Context { x, y, item: next_item(&items, item, true) },
            KeyCode::Enter => {
                self.mode = Mode::Edit;
                return self.act(items[item].action.clone());
            }
            KeyCode::Char(c) => {
                if let Some(chosen) = items.iter().find(|i| i.hotkey == c.to_ascii_lowercase()) {
                    self.mode = Mode::Edit;
                    return self.act(chosen.action.clone());
                }
            }
            _ => {}
        }
        Effect::None
    }

    fn cycle_theme(&mut self, back: bool) {
        const ORDER: [&str; 3] = ["turbo", "mono", "modern"];
        let at = ORDER.iter().position(|t| *t == rust_ide::platform::theme_setting()).unwrap_or(0);
        let next = ORDER[if back { (at + 2) % 3 } else { (at + 1) % 3 }];
        set_theme(next);
        let _ = rust_ide::platform::write_setting("theme", next);
        self.status = format!("Theme: {}", theme_label());
    }

    fn settings_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Settings { selected } = &mut self.mode else { return Effect::None };
        let rows = SETTINGS_ROWS.len();
        let row = *selected;
        match key.code {
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Up | KeyCode::BackTab => *selected = (row + rows - 1) % rows,
            KeyCode::Down | KeyCode::Tab => *selected = (row + 1) % rows,
            KeyCode::Home => *selected = 0,
            KeyCode::End => *selected = rows - 1,
            KeyCode::Left => return self.settings_act(row, true),
            KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => return self.settings_act(row, false),
            KeyCode::Char(c) => {
                if let Some(index) = SETTINGS_ROWS.iter().position(|(_, k)| *k == c.to_ascii_lowercase()) {
                    *selected = index;
                    return self.settings_act(index, false);
                }
            }
            _ => {}
        }
        Effect::None
    }

    /// Changes a row of the Settings dialog (`back`: the other way, for the theme).
    pub fn settings_act(&mut self, row: usize, back: bool) -> Effect {
        match row {
            0 => {
                self.set_preview_mode(if self.preview_mode == PreviewMode::Side { PreviewMode::Tab } else { PreviewMode::Side });
                Effect::None
            }
            1 => {
                let at = WindowStyle::ALL.iter().position(|s| *s == self.window_style).unwrap_or(0);
                let next = WindowStyle::ALL[if back { (at + 2) % 3 } else { (at + 1) % 3 }];
                self.act(Action::SetWindowStyle(next))
            }
            2 => self.act(Action::ToggleVim),
            3 => self.act(Action::ToggleCheckBefore),
            4 => self.act(Action::ToggleLineNumbers),
            5 => {
                self.cycle_theme(back);
                Effect::None
            }
            6 => self.act(Action::ToggleStatusLines),
            7 => self.act(Action::Dependencies),
            _ => {
                self.mode = Mode::Edit;
                Effect::None
            }
        }
    }

    /// Called by the main loop with the result of `deps::check`. At startup the dialog only
    /// opens when something needed is missing.
    pub fn deps_checked(&mut self, list: Vec<rust_ide::deps::Dep>, startup: bool) {
        let missing = list.iter().filter(|d| d.missing() && d.need != rust_ide::deps::Need::Optional).count();
        if startup {
            // Not over a running program or another dialog.
            if missing == 0 || !matches!(self.mode, Mode::Edit) || self.output.visible {
                return;
            }
            self.mode = Mode::Deps(DepsDialog { list: Vec::new(), selected: 0, message: "Checking".into() });
        }
        let Mode::Deps(dialog) = &mut self.mode else { return };
        if dialog.message.starts_with("Checking") {
            dialog.message = if missing == 0 {
                "Everything needed is installed".into()
            } else {
                format!("{missing} required or recommended tool(s) missing")
            };
        }
        dialog.selected = dialog.selected.min(list.len().saturating_sub(1));
        dialog.list = list;
    }

    /// Called by the main loop when an install command finishes; checks again.
    pub fn dep_installed(&mut self, name: &str, result: Result<String, String>) -> Effect {
        let Mode::Deps(dialog) = &mut self.mode else { return Effect::None };
        dialog.message = match result {
            Ok(last) => format!("Installed {name}. {last}"),
            Err(last) => format!("Installing {name} failed: {}", rust_ide::fallback::failure_message(name, &last)),
        };
        Effect::CheckDeps { startup: false }
    }

    fn deps_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Deps(dialog) = &mut self.mode else { return Effect::None };
        let count = dialog.list.len().max(1);
        let dep = dialog.list.get(dialog.selected).cloned();
        match key.code {
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Up | KeyCode::BackTab => dialog.selected = (dialog.selected + count - 1) % count,
            KeyCode::Down | KeyCode::Tab => dialog.selected = (dialog.selected + 1) % count,
            KeyCode::Home => dialog.selected = 0,
            KeyCode::End => dialog.selected = count - 1,
            KeyCode::Char('r' | 'R') => {
                dialog.message = "Checking installed tools...".into();
                return Effect::CheckDeps { startup: false };
            }
            KeyCode::Char('s' | 'S') => {
                self.check_at_startup = !self.check_at_startup;
                let _ = platform::write_setting("check_dependencies", if self.check_at_startup { "true" } else { "false" });
                dialog.message = format!("Check at startup {}", if self.check_at_startup { "on" } else { "off" });
            }
            KeyCode::Char('g' | 'G') => {
                dialog.message = match platform::open_url(platform::INSTALL_GUIDE_URL) {
                    Ok(()) => "Opened the install guide in the browser".into(),
                    Err(e) => format!("Cannot open the browser: {e} ({})", platform::INSTALL_GUIDE_URL),
                };
            }
            KeyCode::Enter | KeyCode::Char('i' | 'I') => {
                let Some(dep) = dep else { return Effect::None };
                match (&dep.found, dep.auto) {
                    (Some(_), _) => dialog.message = format!("{} is installed", dep.name),
                    (None, Some(command)) => {
                        dialog.message = format!("Installing {}: {} ...", dep.name, command.join(" "));
                        return Effect::InstallDep { name: dep.name, command };
                    }
                    (None, None) => dialog.message = "No automatic install: M shows the steps, C copies the command".into(),
                }
            }
            KeyCode::Char('c' | 'C') => {
                let Some(dep) = dep else { return Effect::None };
                dialog.message = format!("Copied: {}", dep.install);
                self.clipboard = dep.install.clone();
                return Effect::Copy(dep.install);
            }
            KeyCode::Char('m' | 'M') => {
                let Some(dep) = dep else { return Effect::None };
                let steps = rust_ide::fallback::manual_steps(dep.name, rust_ide::platform::Os::detect());
                self.mode = Mode::Edit;
                self.new_window();
                *self.buffer_mut() = Buffer::from_text(&steps);
                self.status = format!("Manual steps for {} (Ctrl+A, Ctrl+C copies them)", dep.name);
            }
            _ => {}
        }
        Effect::None
    }

    fn unsaved_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Unsaved { action, button } = &mut self.mode else { return Effect::None };
        let choice = match key.code {
            KeyCode::Left | KeyCode::Up | KeyCode::BackTab => {
                *button = (*button + 2) % 3;
                return Effect::None;
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Tab => {
                *button = (*button + 1) % 3;
                return Effect::None;
            }
            KeyCode::Enter => *button,
            KeyCode::Esc => 2,
            KeyCode::Char(c) => match c.to_ascii_lowercase() {
                's' => 0,
                'd' => 1,
                'c' => 2,
                _ => return Effect::None,
            },
            _ => return Effect::None,
        };
        let action = action.clone();
        self.mode = Mode::Edit;
        self.unsaved_choice(action, choice)
    }

    /// Answers the unsaved-changes dialog: 0 save, 1 discard, 2 cancel.
    pub fn unsaved_choice(&mut self, action: Action, choice: usize) -> Effect {
        match choice {
            2 => self.status = "Cancelled".into(),
            0 if action == Action::Exit => return self.save_all_and_exit(),
            0 => {
                if self.buffer().modified {
                    self.save();
                    // No file name yet (a prompt opened) or the save failed: stay.
                    if self.buffer().modified {
                        return Effect::None;
                    }
                }
                return self.finish_discard(action);
            }
            _ => return self.finish_discard(action),
        }
        Effect::None
    }

    /// Exit + Save: saves every modified file window, then quits. Stops on the window
    /// that needs attention (no file name, or the save failed).
    fn save_all_and_exit(&mut self) -> Effect {
        let home = self.active;
        for index in 0..self.windows.len() {
            self.active = index;
            if !self.buffer().modified || self.buffer().preview_of.is_some() {
                continue;
            }
            if self.buffer().path.is_none() {
                self.save(); // opens the Save As prompt
                self.status = "Name the file, then exit again".into();
                return Effect::None;
            }
            self.save();
            if self.buffer().modified {
                return Effect::None; // the save failed; the status says why
            }
        }
        self.active = home;
        Effect::Quit
    }

    fn finish_discard(&mut self, action: Action) -> Effect {
        match action {
            Action::Exit => Effect::Quit,
            _ => {
                self.close_window();
                Effect::None
            }
        }
    }

    fn menu_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Menu { menu, item } = self.mode else { return Effect::None };
        let items = self.menu_items(menu);
        let step = |by: isize| MENUS[((MENUS.len() as isize + menu.index() as isize + by) as usize) % MENUS.len()];
        match key.code {
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Left => self.mode = Mode::Menu { menu: step(-1), item: 0 },
            KeyCode::Right => self.mode = Mode::Menu { menu: step(1), item: 0 },
            KeyCode::Up => self.mode = Mode::Menu { menu, item: next_item(&items, item, false) },
            KeyCode::Down => self.mode = Mode::Menu { menu, item: next_item(&items, item, true) },
            KeyCode::Enter => {
                self.mode = Mode::Edit;
                return self.act(items[item].action.clone());
            }
            KeyCode::Char(c) => {
                if let Some(chosen) = items.iter().find(|i| i.hotkey == c.to_ascii_lowercase()) {
                    self.mode = Mode::Edit;
                    return self.act(chosen.action.clone());
                }
            }
            _ => {}
        }
        Effect::None
    }

    fn prompt_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Prompt { kind, input } = &mut self.mode else { return Effect::None };
        match key.code {
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(c) => input.push(c),
            KeyCode::Enter => {
                let (kind, input) = (*kind, input.clone());
                self.mode = Mode::Edit;
                return self.submit(kind, &input);
            }
            _ => {}
        }
        Effect::None
    }

    fn open_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Open(dialog) = &mut self.mode else { return Effect::None };
        let count = dialog.visible().len();
        match key.code {
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Up if count > 0 => dialog.selected = (dialog.selected + count - 1) % count,
            KeyCode::Down if count > 0 => dialog.selected = (dialog.selected + 1) % count,
            KeyCode::Char(c) => {
                dialog.filter.push(c);
                dialog.selected = 0;
            }
            KeyCode::Backspace => {
                if dialog.filter.pop().is_none()
                    && let Some(parent) = dialog.dir.parent()
                {
                    dialog.dir = parent.to_path_buf();
                    dialog.read();
                }
                dialog.selected = 0;
            }
            KeyCode::Enter => {
                let Some((name, is_dir)) = dialog.visible().get(dialog.selected).map(|e| (e.0.clone(), e.1)) else {
                    return Effect::None;
                };
                let path = if name == ".." {
                    dialog.dir.parent().map(Path::to_path_buf).unwrap_or_else(|| dialog.dir.clone())
                } else {
                    dialog.dir.join(&name)
                };
                if is_dir {
                    dialog.dir = path;
                    dialog.filter.clear();
                    dialog.read();
                } else {
                    self.mode = Mode::Edit;
                    self.open(&path);
                }
            }
            _ => {}
        }
        Effect::None
    }

    // ----- Actions -----

    pub fn act(&mut self, action: Action) -> Effect {
        self.completion = None;
        match action {
            Action::Separator => {}
            Action::Cut => return self.copy_selection(true),
            Action::Copy => return self.copy_selection(false),
            Action::Paste => return Effect::Paste,
            Action::SelectAll => self.buffer_mut().select_all(),
            Action::RemoveEmptyLines => {
                let selected = self.buffer().selection().is_some();
                let count = self.buffer_mut().remove_empty_lines();
                let place = if selected { " in the selected lines" } else { "" };
                self.status = if count == 0 {
                    format!("No double empty lines{place}")
                } else {
                    format!("Removed {count} extra empty line(s){place} (Ctrl+Z undoes)")
                };
            }
            Action::Settings => self.mode = Mode::Settings { selected: 0 },
            Action::Dependencies => {
                self.mode = Mode::Deps(DepsDialog { list: Vec::new(), selected: 0, message: "Checking installed tools...".into() });
                return Effect::CheckDeps { startup: false };
            }
            Action::CycleTheme => self.cycle_theme(false),
            Action::ToggleStatusLines => {
                self.two_line_status = !self.two_line_status;
                let _ = rust_ide::platform::write_setting("status_lines", if self.two_line_status { "2" } else { "1" });
                self.status = format!("Two-line status {}", if self.two_line_status { "on" } else { "off" });
            }
            Action::New => self.ask_language(false),
            Action::NewProject => self.ask_project(),
            Action::Open => {
                let dir = self
                    .buffer()
                    .path
                    .as_deref()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
                    .or_else(rust_ide::platform::last_dir)
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                self.mode = Mode::Open(OpenDialog::new(dir));
            }
            Action::Save => self.save(),
            Action::SaveAs => {
                let input = self.buffer().path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                self.mode = Mode::Prompt { kind: PromptKind::SaveAs, input };
            }
            Action::Exit => {
                let unsaved = self.windows.iter().filter(|w| w.modified).count();
                if unsaved == 0 {
                    return Effect::Quit;
                }
                self.mode = Mode::Unsaved { action: Action::Exit, button: 0 };
            }
            Action::Undo => {
                if !self.buffer_mut().undo() {
                    self.status = "Nothing to undo".into();
                }
            }
            Action::Redo => {
                if !self.buffer_mut().redo() {
                    self.status = "Nothing to redo".into();
                }
            }
            Action::Find | Action::Replace => {
                let replace = action == Action::Replace;
                // The selected text (one line) is what to look for.
                let query = self.buffer().selected_text().filter(|t| !t.is_empty() && !t.contains('\n'));
                let query = query.unwrap_or_else(|| self.last_query.clone());
                let replacement = self.search.as_ref().map_or_else(|| self.last_replacement.clone(), |b| b.replacement.clone());
                self.search = Some(SearchBar { replace, query, replacement, field: 0 });
                self.status = if replace {
                    "Enter: replace one, Alt+A: replace all, Tab: next field, Alt+M: match case, Esc: close".into()
                } else {
                    "Enter/Down: next match, Up: previous, Alt+M: match case, Esc: close".into()
                };
            }
            Action::FindNext => self.find(true),
            Action::ToggleMatchCase => {
                self.match_case = !self.match_case;
                self.status = format!("Match case {}", if self.match_case { "on" } else { "off" });
            }
            Action::ToggleVim => {
                self.vim = if self.vim.is_some() { None } else { Some(vim::Vim::default()) };
                let _ = platform::write_setting("vim", if self.vim.is_some() { "true" } else { "false" });
                self.status = format!("Vim mode {}", if self.vim.is_some() { "on" } else { "off" });
            }
            Action::GeneralEnv => match rust_ide::env::ensure_general_file() {
                Ok(path) => self.open(&path),
                Err(e) => self.status = format!("Cannot create the general environment file: {e}"),
            },
            Action::ProjectEnv => match rust_ide::env::ensure_project_file(&self.project_dir()) {
                Ok(path) => self.open(&path),
                Err(e) => self.status = format!("Cannot create the project environment file: {e}"),
            },
            Action::PythonSetup => {
                self.status = "Setting up the Python environment (.venv, requirements) ...".into();
                return Effect::PythonSetup(self.project_dir());
            }
            Action::RustToolchain => {
                // `rustup toolchain list` takes a moment, but nothing else is happening.
                let installed = rust_ide::env::installed_toolchains();
                self.open_toolchains(&installed);
            }
            Action::ShowEnv => {
                let text = rust_ide::env::describe(Some(&self.project_dir()));
                self.new_window();
                *self.buffer_mut() = Buffer::from_text(&text);
                self.status = "Environment for programs started by the IDE (read only listing)".into();
            }
            Action::ToggleLineNumbers => {
                self.line_numbers = !self.line_numbers;
                let _ = rust_ide::platform::write_setting("line_numbers", if self.line_numbers { "true" } else { "false" });
                self.status = format!("Line numbers {}", if self.line_numbers { "on" } else { "off" });
            }
            Action::ToggleCheckBefore => {
                self.check_before = !self.check_before;
                let _ = rust_ide::platform::write_setting("check_before", if self.check_before { "true" } else { "false" });
                self.status = format!("Check before run {}", if self.check_before { "on" } else { "off" });
            }
            Action::GoToLine => self.mode = Mode::Prompt { kind: PromptKind::GoTo, input: String::new() },
            Action::Run if self.debug.is_some() => {
                self.debug_command(dap::Command::Continue);
            }
            Action::Run | Action::Compile | Action::RunExternal | Action::DebugRun | Action::GoToCursor => {
                let after = match action {
                    Action::Run => After::Run,
                    Action::RunExternal => After::External,
                    Action::DebugRun => After::Debug(dap::Start::Breakpoints),
                    Action::GoToCursor => After::Debug(dap::Start::ToLine(self.buffer().row)),
                    _ => After::Compile,
                };
                if self.buffer().path.is_none() {
                    self.status = "Save the file first (F2)".into();
                    return Effect::None;
                }
                self.save();
                if self.buffer().modified {
                    return Effect::None; // saving failed
                }
                let path = self.buffer().path.clone().unwrap_or_default();
                if self.check_before {
                    self.then_build = Some(after);
                    self.status = "Checking...".into();
                    return Effect::Check { path };
                }
                self.status = "Compiling...".into();
                return Effect::Build { path, after };
            }
            Action::TraceInto if self.debug.is_none() => {
                return self.act_debug_start(dap::Start::Entry);
            }
            Action::StepOver if self.debug.is_none() => {
                return self.act_debug_start(dap::Start::Entry);
            }
            Action::TraceInto => self.debug_command(dap::Command::StepIn),
            Action::StepOver => self.debug_command(dap::Command::Next),
            Action::StepOut => {
                if self.debug.is_some() {
                    self.debug_command(dap::Command::StepOut);
                } else {
                    self.status = "Not debugging (Shift+F5 starts)".into();
                }
            }
            Action::ToggleBreakpoint => self.toggle_breakpoint(),
            Action::ClearBreakpoints => self.clear_breakpoints(),
            Action::AddWatch => self.mode = Mode::Prompt { kind: PromptKind::Watch, input: self.word_at_cursor() },
            Action::ClearWatches => {
                self.watches.clear();
                self.status = "Watches cleared".into();
            }
            Action::ProgramReset => self.program_reset(),
            Action::TogglePreview => self.toggle_preview(),
            Action::CyclePreviewMode => {
                let next = if self.preview_mode == PreviewMode::Side { PreviewMode::Tab } else { PreviewMode::Side };
                self.set_preview_mode(next);
            }
            Action::SetWindowStyle(style) => {
                self.window_style = style;
                let _ = platform::write_setting("window_style", style.setting());
                self.status = format!("Window style: {}", style.label().to_lowercase());
            }
            Action::OpenUrl(url) => {
                self.status = match platform::open_url(url) {
                    Ok(()) => format!("Opened {url}"),
                    Err(e) => format!("Cannot open the browser: {e} ({url})"),
                };
            }
            Action::SyntaxCheck => {
                if self.buffer().path.is_none() {
                    self.status = "Save the file first (F2)".into();
                    return Effect::None;
                }
                self.save();
                if self.buffer().modified {
                    return Effect::None; // saving failed
                }
                self.then_build = None;
                self.status = "Checking...".into();
                return Effect::Check { path: self.buffer().path.clone().unwrap_or_default() };
            }
            Action::UserScreen => {
                if self.output.title.is_empty() {
                    self.status = "No program has run yet (F5 runs)".into();
                } else {
                    self.output.visible = !self.output.visible;
                }
            }
            Action::NextMessage => self.step_message(true),
            Action::PrevMessage => self.step_message(false),
            Action::NextWindow => self.select((self.active + 1) % self.windows.len()),
            Action::PrevWindow => self.select((self.active + self.windows.len() - 1) % self.windows.len()),
            Action::SelectWindow(index) => self.select(index),
            Action::CloseWindow => {
                if self.buffer().modified && self.buffer().preview_of.is_none() {
                    self.mode = Mode::Unsaved { action: Action::CloseWindow, button: 0 };
                } else {
                    self.close_window();
                }
            }
            Action::Help => self.mode = Mode::Help { scroll: 0 },
            Action::Learn(track) => self.mode = Mode::Lessons { track, selected: 0 },
        }
        Effect::None
    }

    fn submit(&mut self, kind: PromptKind, input: &str) -> Effect {
        match kind {
            PromptKind::SaveAs => {
                if !input.trim().is_empty() {
                    // A name typed without an extension gets the file's type.
                    let ext = match self.buffer().path.as_deref().and_then(Path::extension) {
                        Some(ext) => ext.to_string_lossy().into_owned(),
                        None => self.buffer().ext.unwrap_or(filetype::DEFAULT_EXT).to_string(),
                    };
                    self.buffer_mut().path = Some(PathBuf::from(filetype::with_default_extension(input, &ext)));
                    self.save();
                }
            }
            PromptKind::Watch => self.add_watch(input),
            PromptKind::Toolchain => {
                let name = input.trim();
                let result = rust_ide::env::set_rust_toolchain(&self.project_dir(), (!name.is_empty()).then_some(name));
                self.status = result.unwrap_or_else(|e| e);
            }
            PromptKind::GoTo => match input.trim().parse::<usize>() {
                Ok(line) if line >= 1 => self.buffer_mut().go_to(line - 1, 0),
                _ => self.status = "Not a line number".into(),
            },
        }
        Effect::None
    }

    pub fn save(&mut self) {
        if self.buffer().path.is_none() {
            self.mode = Mode::Prompt { kind: PromptKind::SaveAs, input: String::new() };
            return;
        }
        if self.buffer().preview_of.is_some() {
            self.status = "The preview cannot be saved".into();
            return;
        }
        self.status = match self.buffer_mut().save() {
            Ok(()) => {
                self.sync();
                self.lsp_saved();
                format!("Saved {}", self.buffer().name())
            }
            Err(e) => format!("Cannot save: {e}"),
        };
    }


    // ----- Windows -----

    // ----- New project -----

    pub fn ask_project(&mut self) {
        self.mode = Mode::Project(ProjectDialog {
            lang: templates::last_lang(),
            size: templates::last_size(),
            name: String::new(),
            folder: templates::default_folder(),
            row: PROJECT_LANG,
            error: String::new(),
        });
    }

    fn project_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Project(dialog) = &mut self.mode else { return Effect::None };
        let row = dialog.row;
        let text_row = row == PROJECT_NAME || row == PROJECT_FOLDER;
        match key.code {
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Up => dialog.row = (row + PROJECT_ROWS - 1) % PROJECT_ROWS,
            KeyCode::Down | KeyCode::Tab => dialog.row = (row + 1) % PROJECT_ROWS,
            KeyCode::BackTab => dialog.row = (row + PROJECT_ROWS - 1) % PROJECT_ROWS,
            KeyCode::Enter if row == PROJECT_CREATE + 1 => self.mode = Mode::Edit,
            KeyCode::Enter => self.create_project(),
            KeyCode::Backspace if text_row => {
                let text = if row == PROJECT_NAME { &mut dialog.name } else { &mut dialog.folder };
                text.pop();
                dialog.error.clear();
            }
            KeyCode::Char(c) if text_row => {
                let text = if row == PROJECT_NAME { &mut dialog.name } else { &mut dialog.folder };
                text.push(c);
                dialog.error.clear();
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') if row == PROJECT_LANG || row == PROJECT_SIZE => {
                let back = key.code == KeyCode::Left;
                let step = |len: usize, at: usize| if back { (at + len - 1) % len } else { (at + 1) % len };
                if row == PROJECT_LANG {
                    let at = templates::LANGS.iter().position(|l| *l == dialog.lang).unwrap_or(0);
                    dialog.lang = templates::LANGS[step(templates::LANGS.len(), at)];
                } else {
                    let at = templates::SIZES.iter().position(|s| *s == dialog.size).unwrap_or(0);
                    dialog.size = templates::SIZES[step(templates::SIZES.len(), at)];
                }
            }
            KeyCode::Char(c) => {
                let c = c.to_ascii_lowercase();
                if row == PROJECT_LANG {
                    if let Some(lang) = templates::LANGS.into_iter().find(|l| l.label().to_lowercase().starts_with(c)) {
                        dialog.lang = lang;
                    }
                } else if row == PROJECT_SIZE {
                    if let Some(size) = templates::SIZES.into_iter().find(|s| s.label().to_lowercase().starts_with(c)) {
                        dialog.size = size;
                    }
                } else if c == 'c' {
                    self.create_project();
                }
            }
            _ => {}
        }
        Effect::None
    }

    fn create_project(&mut self) {
        let Mode::Project(dialog) = &mut self.mode else { return };
        let name = dialog.name.trim().to_string();
        let folder = PathBuf::from(dialog.folder.trim());
        match templates::create(dialog.lang, dialog.size, &name, &folder) {
            Err(error) => dialog.error = error,
            Ok(created) => {
                let (lang, size, folder_text) = (dialog.lang, dialog.size, dialog.folder.trim().to_string());
                self.mode = Mode::Edit;
                templates::remember(lang, size, &folder_text);
                self.open(&created.main_file);
                if self.buffer().path.is_some() {
                    self.status = format!("Created {} {} project in {}", size.label(), lang.label(), created.dir.display());
                }
            }
        }
    }

    // ----- Options > Rust toolchain -----

    /// Opens the picker on the current choice.
    pub fn open_toolchains(&mut self, installed: &[String]) {
        let choices = rust_ide::env::toolchain_choices(installed, &self.project_dir());
        let selected = choices.iter().position(|c| c.current).unwrap_or(0);
        self.status = if installed.is_empty() { "rustup found no toolchains (is rustup installed?)".into() } else { String::new() };
        self.mode = Mode::Toolchains { choices, selected };
    }

    fn toolchain_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Toolchains { choices, selected } = &mut self.mode else { return Effect::None };
        let rows = choices.len() + 1;
        match key.code {
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Up => *selected = (*selected + rows - 1) % rows,
            KeyCode::Down | KeyCode::Tab => *selected = (*selected + 1) % rows,
            KeyCode::Home => *selected = 0,
            KeyCode::End => *selected = rows - 1,
            KeyCode::Char('o' | 'O') => *selected = rows - 1,
            KeyCode::Enter => {
                let row = *selected;
                match choices.get(row).cloned() {
                    Some(choice) => {
                        let name = (!choice.name.is_empty()).then_some(choice.name.as_str());
                        self.status = match rust_ide::env::set_rust_toolchain(&self.project_dir(), name) {
                            Ok(message) if choice.usable => message,
                            Ok(message) => format!("{message} - warning: {}", choice.note),
                            Err(e) => e,
                        };
                        self.mode = Mode::Edit;
                    }
                    None => self.mode = Mode::Prompt { kind: PromptKind::Toolchain, input: String::new() },
                }
            }
            _ => {}
        }
        Effect::None
    }

    // ----- Help > Learn: built-in lessons -----

    fn lessons_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Lessons { track, selected } = &mut self.mode else { return Effect::None };
        let count = track.lessons().len();
        match key.code {
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Up | KeyCode::BackTab => *selected = (*selected + count - 1) % count,
            KeyCode::Down | KeyCode::Tab => *selected = (*selected + 1) % count,
            KeyCode::PageUp => *selected = selected.saturating_sub(LESSON_ROWS - 1),
            KeyCode::PageDown => *selected = (*selected + LESSON_ROWS - 1).min(count - 1),
            KeyCode::Home => *selected = 0,
            KeyCode::End => *selected = count - 1,
            KeyCode::Left | KeyCode::Right => {
                let all = Track::ALL;
                let at = all.iter().position(|t| t == track).unwrap_or(0);
                let next = if key.code == KeyCode::Right { (at + 1) % all.len() } else { (at + all.len() - 1) % all.len() };
                *track = all[next];
                *selected = 0;
            }
            KeyCode::Enter => {
                let (track, index) = (*track, *selected);
                self.mode = Mode::Edit;
                match lessons::open(track, index) {
                    Ok(path) => self.open(&path),
                    Err(error) => self.status = format!("Cannot open the lesson: {error}"),
                }
            }
            _ => {}
        }
        Effect::None
    }

    // ----- New file: choose a language -----

    /// Opens the language list, starting on the type chosen last time.
    pub fn ask_language(&mut self, startup: bool) {
        self.mode = Mode::Language { selected: filetype::last_choice(), startup };
    }

    fn language_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Language { selected, startup } = &mut self.mode else { return Effect::None };
        let count = filetype::FILE_TYPES.len();
        match key.code {
            KeyCode::Esc => {
                // At start-up: a plain Rust window; from File > New: nothing.
                let startup = *startup;
                self.mode = Mode::Edit;
                if startup {
                    self.new_window_of(filetype::DEFAULT_EXT);
                }
            }
            KeyCode::Up => *selected = (*selected + count - 1) % count,
            KeyCode::Down | KeyCode::Tab => *selected = (*selected + 1) % count,
            KeyCode::Home => *selected = 0,
            KeyCode::End => *selected = count - 1,
            KeyCode::Enter => {
                let index = *selected;
                self.choose_language(index);
            }
            KeyCode::Char(c) => {
                if let Some(index) = filetype::by_key(c) {
                    self.choose_language(index);
                }
            }
            _ => {}
        }
        Effect::None
    }

    fn choose_language(&mut self, index: usize) {
        let ext = filetype::ext_of(index);
        self.mode = Mode::Edit;
        filetype::remember(ext);
        self.new_window_of(ext);
    }

    /// A new untitled window of this type (an empty untitled one is reused).
    fn new_window_of(&mut self, ext: &'static str) {
        if self.buffer().is_blank() {
            self.buffer_mut().ext = Some(ext);
        } else {
            self.windows.insert(self.active + 1, Buffer::untitled(ext));
            self.active += 1;
        }
        self.status = format!("New file ({})", filetype::untitled_name(Some(ext)));
    }

    /// The folder the project environment belongs to: the open file's, else the working folder.
    /// "Rust nightly " for the status line while a Rust file is open.
    pub fn toolchain_label(&self) -> String {
        match self.buffer().path.as_deref() {
            Some(path) if path.extension().is_some_and(|e| e == "rs") => {
                format!("{} ", rust_ide::env::toolchain_label(&self.project_dir()))
            }
            _ => String::new(),
        }
    }

    fn project_dir(&self) -> PathBuf {
        self.buffer()
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    }

    fn new_window(&mut self) {
        self.windows.insert(self.active + 1, Buffer::default());
        self.active += 1;
    }

    pub fn open(&mut self, path: &Path) {
        let same = |p: &Path| rust_ide::lsp::same_file(p, path);
        if let Some(index) = self.windows.iter().position(|w| w.path.as_deref().is_some_and(same)) {
            self.select(index);
            return;
        }
        match Buffer::open(path) {
            Ok(buffer) => {
                if self.buffer().is_blank() {
                    self.windows[self.active] = buffer;
                } else {
                    self.windows.insert(self.active + 1, buffer);
                    self.active += 1;
                }
                if self.remember_dir {
                    rust_ide::platform::remember_dir(path);
                }
                self.status = format!("Opened {}", path.display());
            }
            Err(e) => self.status = format!("Cannot open {}: {e}", path.display()),
        }
    }

    fn select(&mut self, index: usize) {
        if index < self.windows.len() {
            self.active = index;
            self.status = format!("Window {}: {}", index + 1, self.buffer().name());
        }
    }

    fn close_window(&mut self) {
        if self.windows.len() == 1 {
            self.windows[0] = Buffer::default();
        } else {
            self.windows.remove(self.active);
            self.active = self.active.min(self.windows.len() - 1);
        }
        self.status = format!("{} window(s) open", self.windows.len());
    }

    // ----- Build results -----

    /// Called by the main loop when a build finishes.
    pub fn build_finished(&mut self, result: BuildResult, after: After) -> Effect {
        self.messages = result.diagnostics;
        self.message_index = None;
        let errors = self.messages.iter().filter(|d| d.level == "error").count();
        let warnings = self.messages.len() - errors;
        if !result.success {
            self.status = format!("Compile failed: {errors} error(s), {warnings} warning(s)");
            self.step_message(true);
            return Effect::None;
        }
        self.status = format!("Compile successful, {warnings} warning(s)");
        let Some(target) = result.target else { return Effect::None };
        if after != After::Compile {
            self.last_run = Some(self.buffer().name());
        }
        match after {
            After::Compile => Effect::None,
            After::Run => self.start_run(target),
            After::External => match runner::run_external(&target) {
                Ok(()) => {
                    self.status =
                        if target.gui { "Running (GUI window)".into() } else { "Running in an external terminal".into() };
                    Effect::None
                }
                // No terminal window can be opened (e.g. over SSH): run it here instead.
                Err(_) => Effect::RunHere(target),
            },
            After::Debug(start) => self.start_debug(target, start),
        }
    }

    /// Trace into / Step over before a session: build, then stop at the start.
    fn act_debug_start(&mut self, start: dap::Start) -> Effect {
        let Some(path) = self.buffer().path.clone() else {
            self.status = "Save the file first (F2)".into();
            return Effect::None;
        };
        self.save();
        if self.buffer().modified {
            return Effect::None;
        }
        self.then_build = None;
        self.status = "Compiling...".into();
        Effect::Build { path, after: After::Debug(start) }
    }

    /// The identifier under the cursor (for Add watch).
    fn word_at_cursor(&self) -> String {
        let b = self.buffer();
        let line = &b.lines[b.row];
        let start = line[..b.col].trim_end_matches(|c: char| is_ident(c)).len();
        let end = b.col + line[b.col..].len() - line[b.col..].trim_start_matches(|c: char| is_ident(c)).len();
        line[start..end].to_string()
    }

    /// The Messages panel: the last build's messages, else the language server's.
    pub fn panel_items(&self) -> Vec<Diagnostic> {
        if !self.messages.is_empty() {
            return self.messages.clone();
        }
        self.live
            .iter()
            .map(|d| Diagnostic {
                file: self.buffer().path.clone(),
                line: d.line + 1,
                column: d.start + 1,
                level: if d.error { "error" } else { "warning" }.to_string(),
                message: d.message.clone(),
            })
            .collect()
    }

    /// Called by the main loop when a syntax check finishes.
    pub fn check_finished(&mut self, result: BuildResult) -> Effect {
        let then = self.then_build.take();
        self.messages = result.diagnostics;
        self.message_index = None;
        let errors = self.messages.iter().filter(|d| d.level == "error").count();
        let warnings = self.messages.len() - errors;
        if !result.success {
            self.status = format!("Syntax check failed: {errors} error(s), {warnings} warning(s)");
            self.step_message(true);
        } else if warnings == 0 {
            self.status = "No syntax errors".into();
        } else {
            self.status = format!("No syntax errors, {warnings} warning(s)");
        }
        match then {
            Some(after) if result.success => {
                let path = self.buffer().path.clone().unwrap_or_default();
                self.status.push_str("; compiling...");
                Effect::Build { path, after }
            }
            _ => Effect::None,
        }
    }

    // ----- Mouse -----

    /// A mouse event on a screen of the given size: menus, tabs and the editor.
    pub fn mouse(&mut self, event: MouseEvent, screen: Rect) -> Effect {
        let editor = crate::ui::editor_area(self, screen);
        let (x, y) = (event.column, event.row);
        if matches!(event.kind, MouseEventKind::Down(_)) {
            self.completion = None;
        }
        // The text position under the mouse (clamped to the editor; rows past it scroll).
        let place = |b: &Buffer| {
            let row = (b.scroll as i64 + i64::from(y) - i64::from(editor.y)).max(0) as usize;
            (row, b.hscroll + x.saturating_sub(editor.x) as usize)
        };
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) if matches!(self.mode, Mode::Context { .. }) => {
                let chosen = crate::ui::context_item_at(self, screen, x, y);
                self.mode = Mode::Edit;
                if let Some(index) = chosen {
                    return self.act(context_items()[index].action.clone());
                }
            }
            MouseEventKind::Down(MouseButton::Right) if matches!(self.mode, Mode::Edit | Mode::Context { .. }) => {
                if editor.contains(Position::new(x, y)) {
                    if self.buffer().selection().is_none() {
                        let (row, col) = place(self.buffer());
                        self.buffer_mut().go_to(row, col);
                    }
                    self.mode = Mode::Context { x, y, item: 0 };
                }
            }
            MouseEventKind::Drag(MouseButton::Left) if matches!(self.mode, Mode::Edit) => {
                if let Some(from) = self.drag_from {
                    let (row, col) = place(self.buffer());
                    let b = self.buffer_mut();
                    if b.anchor.is_none() {
                        b.go_to(from.0, from.1);
                        b.start_selection();
                    }
                    b.move_to(row, col);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => self.drag_from = None,
            MouseEventKind::Down(MouseButton::Left) => {
                self.drag_from = None;
                if y == screen.y {
                    self.mode = match crate::ui::menu_at(x) {
                        Some(menu) if !matches!(self.mode, Mode::Menu { menu: open, .. } if open == menu) => {
                            Mode::Menu { menu, item: 0 }
                        }
                        _ => Mode::Edit,
                    };
                    return Effect::None;
                }
                if let Mode::Menu { menu, .. } = self.mode {
                    self.mode = Mode::Edit;
                    if let Some(index) = crate::ui::dropdown_item_at(self, menu, screen, x, y) {
                        let action = self.menu_items(menu)[index].action.clone();
                        return self.act(action);
                    }
                    return Effect::None;
                }
                if !matches!(self.mode, Mode::Edit) {
                    return Effect::None;
                }
                if self.output.visible {
                    return Effect::None;
                }
                if let Some(index) = crate::ui::window_at(self, screen, x, y) {
                    self.select(index);
                } else if editor.contains(Position::new(x, y)) && self.buffer().preview_of.is_none() {
                    let (row, col) = place(self.buffer());
                    let shift = event.modifiers.contains(KeyModifiers::SHIFT);
                    let b = self.buffer_mut();
                    if shift {
                        // Shift+click extends the selection.
                        b.start_selection();
                        b.move_to(row, col);
                    } else {
                        b.go_to(row, col);
                        self.drag_from = Some((b.row, b.char_col()));
                    }
                }
            }
            MouseEventKind::ScrollUp if matches!(self.mode, Mode::Edit) => {
                self.buffer_mut().clear_selection();
                self.buffer_mut().vertical(-3);
            }
            MouseEventKind::ScrollDown if matches!(self.mode, Mode::Edit) => {
                self.buffer_mut().clear_selection();
                self.buffer_mut().vertical(3);
            }
            _ => {}
        }
        Effect::None
    }

    fn step_message(&mut self, forward: bool) {
        let items = self.panel_items();
        if items.is_empty() {
            self.status = "No messages".into();
            return;
        }
        let count = items.len();
        let index = match self.message_index {
            Some(i) if forward => (i + 1) % count,
            Some(i) => (i + count - 1) % count,
            None if forward => 0,
            None => count - 1,
        };
        self.message_index = Some(index);
        let message = items[index].clone();
        if let Some(file) = &message.file {
            self.open(file);
        }
        if message.line > 0 {
            self.buffer_mut().go_to(message.line - 1, message.column.saturating_sub(1));
        }
        self.status = format!("{} {}/{}: {}", message.level.to_uppercase(), index + 1, count, message.message);
    }
}

/// Keys that work everywhere, Turbo Pascal style.
fn global_key(key: &KeyEvent) -> Option<Action> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    Some(match key.code {
        KeyCode::F(1) => Action::Help,
        KeyCode::F(2) if ctrl => Action::ProgramReset,
        KeyCode::F(2) => Action::Save,
        KeyCode::F(3) if alt || ctrl => Action::CloseWindow,
        KeyCode::F(3) => Action::Open,
        KeyCode::F(4) => Action::GoToCursor,
        KeyCode::F(5) if alt => Action::UserScreen,
        KeyCode::F(5) if shift => Action::DebugRun,
        KeyCode::F(5) => Action::Run,
        KeyCode::F(6) if alt => Action::TogglePreview,
        KeyCode::F(6) if shift => Action::PrevWindow,
        KeyCode::F(6) => Action::NextWindow,
        KeyCode::F(7) if alt => Action::PrevMessage,
        KeyCode::F(7) if ctrl => Action::AddWatch,
        KeyCode::F(7) => Action::TraceInto,
        KeyCode::F(8) if ctrl => Action::ToggleBreakpoint,
        KeyCode::F(8) if shift => Action::StepOut,
        KeyCode::F(8) if alt => Action::NextMessage,
        KeyCode::F(8) => Action::StepOver,
        KeyCode::F(9) if ctrl => Action::Run,
        KeyCode::F(9) if alt => Action::SyntaxCheck,
        KeyCode::F(9) => Action::Compile,
        KeyCode::Char(c) if ctrl && !alt => match c.to_ascii_lowercase() {
            's' => Action::Save,
            'o' => Action::Open,
            'z' => Action::Undo,
            'y' => Action::Redo,
            'x' => Action::Cut,
            'c' => Action::Copy,
            'v' => Action::Paste,
            'a' => Action::SelectAll,
            'f' => Action::Find,
            'l' => Action::FindNext,
            // Ctrl+H is Backspace in many terminals, so Ctrl+R also replaces.
            'h' | 'r' => Action::Replace,
            'g' => Action::GoToLine,
            'q' => Action::Exit,
            _ => return None,
        },
        KeyCode::Char(c) if alt && !ctrl => match c.to_ascii_lowercase() {
            'x' => Action::Exit,
            'm' => Action::ToggleMatchCase,
            'p' => Action::TogglePreview,
            d @ '1'..='9' => Action::SelectWindow(d as usize - '1' as usize),
            _ => return None,
        },
        _ => return None,
    })
}

/// F10 and Alt+letter open menus (handled before global keys).
pub fn menu_key(key: &KeyEvent) -> Option<Menu> {
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::F(10) => Some(Menu::File),
        KeyCode::Char('0') if alt => Some(Menu::Window),
        KeyCode::Char(c) if alt && c != 'x' && !c.is_ascii_digit() => Menu::from_hotkey(c),
        _ => None,
    }
}

impl App {
    /// Entry point for every key: menus first, then everything else.
    pub fn handle(&mut self, key: KeyEvent) -> Effect {
        if let Some(menu) = menu_key(&key) {
            self.mode = Mode::Menu { menu, item: 0 };
            return Effect::None;
        }
        self.key(key)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn with(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn typed(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle(if c == '\n' { key(KeyCode::Enter) } else { key(KeyCode::Char(c)) });
        }
    }

    fn temp_files(names: &[&str]) -> Vec<PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("tui-tests");
        std::fs::create_dir_all(&dir).unwrap();
        names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, format!("// {n}\nfn main() {{}}\n")).unwrap();
                p
            })
            .collect()
    }

    #[test]
    fn menus_with_keys() {
        let mut app = App::new(&[]);
        app.handle(with(KeyCode::Char('s'), KeyModifiers::ALT));
        assert!(matches!(app.mode, Mode::Menu { menu: Menu::Search, item: 0 }));
        app.handle(key(KeyCode::Down));
        app.handle(key(KeyCode::Down));
        app.handle(key(KeyCode::Down));
        app.handle(key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::Prompt { kind: PromptKind::GoTo, .. }));
        app.handle(key(KeyCode::Esc));
        app.handle(key(KeyCode::F(10)));
        app.handle(key(KeyCode::Left));
        assert!(matches!(app.mode, Mode::Menu { menu: Menu::Help, .. }), "Left wraps from File to Help");
        app.handle(key(KeyCode::Char('k')));
        assert!(matches!(app.mode, Mode::Help { .. }), "hotkey letter runs the item");
    }

    #[test]
    fn find_and_go_to_line() {
        let mut app = App::new(&[]);
        typed(&mut app, "alpha\nbeta alpha\ngamma");
        app.handle(with(KeyCode::Home, KeyModifiers::CONTROL));
        app.handle(with(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert!(app.search.is_some(), "Ctrl+F opens the search bar");
        typed(&mut app, "alpha\n");
        assert_eq!((app.buffer().row, app.buffer().char_col()), (0, 5));
        assert_eq!(app.buffer().selected_text().as_deref(), Some("alpha"), "the match is selected");
        assert_eq!(app.status, "Match 1 of 2");
        app.handle(key(KeyCode::Down));
        assert_eq!((app.buffer().row, app.buffer().char_col()), (1, 10));
        app.handle(key(KeyCode::Esc));
        assert!(app.search.is_none());
        app.handle(with(KeyCode::Char('l'), KeyModifiers::CONTROL));
        assert_eq!((app.buffer().row, app.buffer().char_col()), (0, 5), "wraps around");
        assert!(app.status.ends_with("(wrapped)"));
        app.handle(with(KeyCode::Char('f'), KeyModifiers::CONTROL));
        app.handle(key(KeyCode::Up));
        assert_eq!(app.buffer().row, 1, "Up finds the previous match");
        // Match case.
        app.handle(with(KeyCode::Char('m'), KeyModifiers::ALT));
        assert!(app.match_case);
        for _ in 0..5 {
            app.handle(key(KeyCode::Backspace));
        }
        typed(&mut app, "ALPHA\n");
        assert!(app.status.contains("not found"), "{}", app.status);
        app.handle(key(KeyCode::Esc));
        app.handle(with(KeyCode::Char('g'), KeyModifiers::CONTROL));
        typed(&mut app, "3\n");
        assert_eq!(app.buffer().row, 2);
    }

    #[test]
    fn windows_open_switch_and_close() {
        let files = temp_files(&["w1.rs", "w2.rs"]);
        let mut app = App::new(&files);
        assert_eq!(app.windows.len(), 2);
        assert_eq!(app.active, 0);
        app.handle(key(KeyCode::F(6)));
        assert_eq!(app.active, 1);
        typed(&mut app, "x");
        app.handle(with(KeyCode::F(3), KeyModifiers::ALT));
        assert_eq!(app.windows.len(), 2, "unsaved: Alt+F3 asks");
        assert!(matches!(app.mode, Mode::Unsaved { .. }));
        app.handle(key(KeyCode::Esc));
        assert_eq!(app.windows.len(), 2, "Cancel keeps the window");
        app.handle(with(KeyCode::F(3), KeyModifiers::ALT));
        typed(&mut app, "d");
        assert_eq!(app.windows.len(), 1);
        app.open(&files[0]);
        assert_eq!(app.windows.len(), 1, "an open file is not opened twice");
    }

    fn two_windows(names: &[&str]) -> (App, Vec<std::path::PathBuf>) {
        let files = temp_files(names);
        let app = App::new(&files);
        (app, files)
    }

    #[test]
    fn exit_save_saves_every_window() {
        let (mut app, files) = two_windows(&["xs1.rs", "xs2.rs"]);
        typed(&mut app, "x"); // window 0 is active and modified
        app.handle(key(KeyCode::F(6)));
        assert_eq!(app.handle(with(KeyCode::Char('x'), KeyModifiers::ALT)), Effect::None);
        assert!(matches!(app.mode, Mode::Unsaved { .. }));
        assert_eq!(app.handle(key(KeyCode::Char('s'))), Effect::Quit);
        assert!(std::fs::read_to_string(&files[0]).unwrap().contains('x'));
        assert_eq!(app.active, 1, "the window that was active stays active");
    }

    #[test]
    fn exit_save_stops_at_an_untitled_window() {
        let (mut app, _files) = two_windows(&["xu1.rs"]);
        app.new_window();
        typed(&mut app, "x");
        app.handle(key(KeyCode::F(6)));
        let named = app.active;
        app.handle(with(KeyCode::Char('x'), KeyModifiers::ALT));
        assert_eq!(app.handle(key(KeyCode::Char('s'))), Effect::None);
        assert_ne!(app.active, named);
        assert!(app.buffer().path.is_none());
        assert!(matches!(app.mode, Mode::Prompt { .. }));
    }

    #[test]
    fn vim_quit_looks_at_every_window() {
        for command in [":q\n", ":wq\n"] {
            let (mut app, _files) = two_windows(&["vq1.rs", "vq2.rs"]);
            app.vim = Some(vim::Vim::default());
            typed(&mut app, "x"); // Vim normal mode: x deletes a character and marks it modified
            assert!(app.buffer().modified, "window 0 modified");
            app.handle(key(KeyCode::F(6)));
            let mut effect = Effect::None;
            for c in command.chars() {
                effect = app.handle(if c == '\n' { key(KeyCode::Enter) } else { key(KeyCode::Char(c)) });
            }
            assert_eq!(effect, Effect::None);
            assert!(matches!(app.mode, Mode::Unsaved { action: Action::Exit, .. }), "{command:?}");
        }
    }

    #[test]
    fn dialog_blocks_window_switching() {
        let (mut app, _files) = two_windows(&["bw1.rs", "bw2.rs"]);
        typed(&mut app, "x");
        app.handle(with(KeyCode::F(3), KeyModifiers::ALT));
        assert!(matches!(app.mode, Mode::Unsaved { .. }));
        app.handle(key(KeyCode::F(6)));
        assert!(matches!(app.mode, Mode::Unsaved { .. }));
        assert_eq!(app.active, 0);
    }

    #[test]
    fn exit_asks_with_a_dialog_for_unsaved_changes() {
        let mut app = App::new(&[]);
        typed(&mut app, "x");
        assert_eq!(app.handle(with(KeyCode::Char('x'), KeyModifiers::ALT)), Effect::None);
        assert!(matches!(app.mode, Mode::Unsaved { .. }));
        assert_eq!(app.handle(key(KeyCode::Tab)), Effect::None);
        assert_eq!(app.handle(key(KeyCode::Enter)), Effect::Quit, "Tab moved to Discard");
    }

    #[test]
    fn compile_needs_a_file_and_reports_errors() {
        let mut app = App::new(&[]);
        assert_eq!(app.handle(key(KeyCode::F(9))), Effect::None);
        assert!(matches!(app.mode, Mode::Prompt { kind: PromptKind::SaveAs, .. }) || app.status.contains("Save"));

        let files = temp_files(&["broken.rs"]);
        let mut app = App::new(&files);
        let effect = app.handle(key(KeyCode::F(9)));
        assert_eq!(effect, Effect::Build { path: files[0].clone(), after: After::Compile });
        let result = BuildResult {
            success: false,
            diagnostics: vec![Diagnostic {
                file: Some(files[0].clone()),
                line: 2,
                column: 4,
                level: "error".into(),
                message: "oops".into(),
            }],
            target: None,
        };
        assert_eq!(app.build_finished(result, After::Compile), Effect::None);
        assert_eq!((app.buffer().row, app.buffer().char_col()), (1, 3), "jumps to the first error");
        assert!(app.status.starts_with("ERROR 1/1"));
    }

    #[test]
    fn alt_f9_is_the_syntax_check_and_f9_still_compiles() {
        let files = temp_files(&["check_me.rs"]);
        let mut app = App::new(&files);
        assert_eq!(app.handle(with(KeyCode::F(9), KeyModifiers::ALT)), Effect::Check { path: files[0].clone() });
        assert_eq!(app.handle(key(KeyCode::F(9))), Effect::Build { path: files[0].clone(), after: After::Compile });
        assert_eq!(
            app.handle(with(KeyCode::F(9), KeyModifiers::CONTROL)),
            Effect::Build { path: files[0].clone(), after: After::Run }
        );

        // From the Compile menu: the item has the 's' hotkey.
        app.handle(with(KeyCode::Char('c'), KeyModifiers::ALT));
        assert!(matches!(app.mode, Mode::Menu { menu: Menu::Compile, .. }));
        assert_eq!(app.handle(key(KeyCode::Char('s'))), Effect::Check { path: files[0].clone() });

        let mut unsaved = App::new(&[]);
        assert_eq!(unsaved.handle(with(KeyCode::F(9), KeyModifiers::ALT)), Effect::None);
    }

    #[test]
    fn syntax_check_results() {
        let files = temp_files(&["check_result.rs"]);
        let mut app = App::new(&files);
        let ok = BuildResult { success: true, diagnostics: Vec::new(), target: None };
        app.check_finished(ok);
        assert_eq!(app.status, "No syntax errors");
        assert!(app.messages.is_empty());

        let warning = Diagnostic {
            file: Some(files[0].clone()),
            line: 1,
            column: 1,
            level: "warning".into(),
            message: "unused".into(),
        };
        app.check_finished(BuildResult { success: true, diagnostics: vec![warning], target: None });
        assert_eq!(app.status, "No syntax errors, 1 warning(s)");

        let error = Diagnostic {
            file: Some(files[0].clone()),
            line: 2,
            column: 5,
            level: "error".into(),
            message: "bad".into(),
        };
        app.check_finished(BuildResult { success: false, diagnostics: vec![error], target: None });
        assert!(app.status.starts_with("ERROR 1/1: bad"), "{}", app.status);
        assert_eq!((app.buffer().row, app.buffer().char_col()), (1, 4), "jumps to the error");
    }

    /// Keeps these tests away from the real settings file, and makes the tests that
    /// use it take turns (the settings file is shared by the whole test process).
    pub(crate) fn isolate_settings() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        static ONCE: std::sync::Once = std::sync::Once::new();
        let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        ONCE.call_once(|| {
            let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("tui-language-settings.txt");
            let _ = std::fs::remove_file(&file);
            // SAFETY: set once, before any test here reads settings.
            unsafe { std::env::set_var("RUST_IDE_CONFIG_FILE", file) };
        });
        guard
    }

    #[test]
    fn help_learn_lists_lessons_and_opens_one() {
        let folder = std::env::temp_dir().join(format!("rust-ide-tui-lessons-{}", std::process::id()));
        // Only this test uses the variable in this test binary.
        unsafe { std::env::set_var("RUST_IDE_LESSONS_DIR", &folder) };
        let mut app = App::new(&[]);
        app.handle(with(KeyCode::Char('h'), KeyModifiers::ALT));
        assert!(matches!(app.mode, Mode::Menu { menu: Menu::Help, .. }));
        app.handle(key(KeyCode::Char('y')));
        assert!(matches!(app.mode, Mode::Lessons { track: Track::Python, selected: 0 }));
        app.handle(key(KeyCode::End));
        assert!(matches!(app.mode, Mode::Lessons { selected: 24, .. }));
        app.handle(key(KeyCode::Down));
        assert!(matches!(app.mode, Mode::Lessons { selected: 0, .. }), "wraps round");
        app.handle(key(KeyCode::Right));
        assert!(matches!(app.mode, Mode::Lessons { track: Track::CSharp, .. }));
        app.handle(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Edit));

        app.act(Action::Learn(Track::Rust));
        for _ in 0..6 {
            app.handle(key(KeyCode::Down));
        }
        app.handle(key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::Edit));
        let path = app.buffer().path.clone().expect("the lesson is open");
        assert!(path.ends_with("07_ownership.rs"), "{}", path.display());
        assert!(path.starts_with(&folder));
        assert_eq!(app.windows.len(), 1, "the empty start window is reused");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn startup_chooser_keys() {
        let _settings = isolate_settings();
        let mut app = App::new(&[]);
        app.ask_language(true);
        assert!(matches!(app.mode, Mode::Language { startup: true, .. }));
        app.handle(key(KeyCode::Home));
        app.handle(key(KeyCode::Down));
        app.handle(key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::Edit));
        assert_eq!(app.buffer().ext, Some("py"));
        assert_eq!(app.buffer().name(), "NONAME.PY");
        assert_eq!(app.buffer().lang(), rust_ide::highlight::Lang::Python);
        assert_eq!(app.windows.len(), 1, "the empty start window is reused");

        app.ask_language(false);
        app.handle(key(KeyCode::Char('m')));
        assert_eq!((app.buffer().ext, app.buffer().lang()), (Some("md"), rust_ide::highlight::Lang::Plain));
        app.ask_language(false);
        app.handle(key(KeyCode::Char('4')));
        assert_eq!(app.buffer().name(), "NONAME.TXT");
        app.ask_language(false);
        app.handle(key(KeyCode::Char('z')));
        assert!(matches!(app.mode, Mode::Language { .. }), "other keys leave the list open");
    }

    #[test]
    fn escape_and_file_new() {
        let _settings = isolate_settings();
        let mut app = App::new(&[]);
        app.ask_language(true);
        app.handle(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Edit));
        assert_eq!(app.buffer().ext, Some("rs"), "a plain Rust window");

        typed(&mut app, "x");
        // File > New (menu item 'n') asks for the language and opens a new window.
        app.handle(with(KeyCode::Char('f'), KeyModifiers::ALT));
        app.handle(key(KeyCode::Char('n')));
        assert!(matches!(app.mode, Mode::Language { startup: false, .. }));
        app.handle(key(KeyCode::Esc));
        assert_eq!(app.windows.len(), 1, "cancelled");
        app.handle(with(KeyCode::Char('f'), KeyModifiers::ALT));
        app.handle(key(KeyCode::Char('n')));
        app.handle(key(KeyCode::Char('p')));
        assert_eq!(app.windows.len(), 2);
        assert_eq!(app.active, 1);
        assert_eq!(app.buffer().name(), "NONAME.PY");
        assert_eq!(app.windows[0].name(), "NONAME.RS");
    }

    #[test]
    fn the_last_choice_is_preselected() {
        let _settings = isolate_settings();
        let mut app = App::new(&[]);
        app.ask_language(false);
        app.handle(key(KeyCode::Char('m')));
        app.ask_language(false);
        assert!(matches!(app.mode, Mode::Language { selected: 4, .. }));
        app.handle(key(KeyCode::Enter));
        assert_eq!(app.buffer().ext, Some("md"));
    }

    #[test]
    fn the_open_dialog_starts_in_the_folder_used_last() {
        let _settings = isolate_settings();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("tui-tests").join("lastdir");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.txt");
        std::fs::write(&file, "x").unwrap();
        let mut app = App::new(&[]);
        app.remember_dir = true;
        app.open(&file);
        assert_eq!(rust_ide::platform::last_dir().as_deref(), Some(dir.as_path()));
        let mut other = App::new(&[]);
        other.act(Action::Open);
        let Mode::Open(dialog) = &other.mode else { panic!() };
        assert_eq!(dialog.dir, dir);
    }

    #[test]
    fn save_as_adds_the_chosen_extension() {
        let _settings = isolate_settings();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("tui-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let _ = std::fs::remove_file(dir.join("saved_name.py"));
        let mut app = App::new(&[]);
        app.ask_language(false);
        app.handle(key(KeyCode::Char('p')));
        typed(&mut app, "print(1)");
        app.handle(key(KeyCode::F(2)));
        assert!(matches!(app.mode, Mode::Prompt { kind: PromptKind::SaveAs, .. }));
        typed(&mut app, &format!("{}\n", dir.join("saved_name").display()));
        assert!(dir.join("saved_name.py").is_file(), "typed without an extension: .py is added");
        assert!(!app.buffer().modified);
        assert_eq!(app.buffer().name(), "saved_name.py");
    }

    #[test]
    fn open_dialog_lists_and_filters() {
        let files = temp_files(&["pick_me.rs", "other.py"]);
        let mut app = App::new(&[]);
        app.mode = Mode::Open(OpenDialog::new(files[0].parent().unwrap().to_path_buf()));
        typed(&mut app, "pick");
        let Mode::Open(dialog) = &app.mode else { panic!() };
        assert_eq!(dialog.visible().len(), 1);
        app.handle(key(KeyCode::Enter));
        assert_eq!(app.buffer().name(), "pick_me.rs");
    }

    #[test]
    fn new_project_dialog_chooses_and_creates() {
        let _settings = isolate_settings();
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("tui-project-test");
        let _ = std::fs::remove_dir_all(&parent);
        std::fs::create_dir_all(&parent).unwrap();
        let mut app = App::new(&[]);
        // File > New project... (menu hotkey P).
        app.handle(with(KeyCode::Char('f'), KeyModifiers::ALT));
        app.handle(key(KeyCode::Char('p')));
        assert!(matches!(app.mode, Mode::Project(_)));
        app.handle(key(KeyCode::Char('p'))); // language: Python
        app.handle(key(KeyCode::Down));
        app.handle(key(KeyCode::Char('m'))); // size: Medium
        app.handle(key(KeyCode::Down));
        typed(&mut app, "tui-demo"); // name
        app.handle(key(KeyCode::Down));
        for _ in 0..app_folder_len(&app) {
            app.handle(key(KeyCode::Backspace));
        }
        typed(&mut app, &parent.to_string_lossy());
        {
            let Mode::Project(d) = &app.mode else { panic!() };
            assert_eq!((d.lang, d.size), (Lang::Python, Size::Medium));
            assert_eq!(d.name, "tui-demo");
        }
        app.handle(key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::Edit), "created");
        let main = parent.join("tui-demo").join("src").join("tui_demo").join("__main__.py");
        assert!(main.is_file());
        assert_eq!(app.buffer().path.as_deref(), Some(main.as_path()));
        assert_eq!(templates::last_size(), Size::Medium);
    }

    fn app_folder_len(app: &App) -> usize {
        let Mode::Project(d) = &app.mode else { panic!() };
        d.folder.chars().count()
    }

    #[test]
    fn new_project_dialog_reports_problems_and_cancels() {
        let _settings = isolate_settings();
        let mut app = App::new(&[]);
        app.ask_project();
        app.handle(key(KeyCode::Down));
        app.handle(key(KeyCode::Down));
        typed(&mut app, "9lives");
        app.handle(key(KeyCode::Enter));
        let Mode::Project(d) = &app.mode else { panic!("still open") };
        assert!(d.error.contains("letter"));
        app.handle(key(KeyCode::Char('x')));
        let Mode::Project(d) = &app.mode else { panic!() };
        assert!(d.error.is_empty(), "typing clears the message");
        app.handle(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Edit));
    }

    #[test]
    fn replace_bar_replaces_one_or_all_and_undoes() {
        let mut app = App::new(&[]);
        typed(&mut app, "Foo bar foo\nFOO");
        app.handle(with(KeyCode::Home, KeyModifiers::CONTROL));
        app.handle(with(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(app.search.as_ref().is_some_and(|b| b.replace), "Ctrl+R opens the replace bar");
        typed(&mut app, "foo");
        app.handle(key(KeyCode::Tab));
        typed(&mut app, "x");
        // Enter in the Replace field: the first Enter finds, the next replaces it.
        app.handle(key(KeyCode::Enter));
        assert_eq!(app.buffer().selected_text().as_deref(), Some("Foo"));
        app.handle(key(KeyCode::Enter));
        assert_eq!(app.buffer().text(), "x bar foo\nFOO", "case is ignored unless Alt+M");
        app.handle(with(KeyCode::Char('a'), KeyModifiers::ALT));
        assert_eq!(app.buffer().text(), "x bar x\nx");
        assert!(app.status.contains("Replaced 2"), "{}", app.status);
        app.handle(key(KeyCode::Esc));
        app.handle(with(KeyCode::Char('z'), KeyModifiers::CONTROL));
        assert_eq!(app.buffer().text(), "x bar foo\nFOO", "one undo per replace");
        // Match case: only the exact text.
        app.handle(with(KeyCode::Char('r'), KeyModifiers::CONTROL));
        app.handle(with(KeyCode::Char('m'), KeyModifiers::ALT));
        app.handle(with(KeyCode::Char('a'), KeyModifiers::ALT));
        assert_eq!(app.buffer().text(), "x bar x\nFOO");
        // Nothing found.
        for _ in 0..3 {
            app.handle(key(KeyCode::Backspace));
        }
        typed(&mut app, "zzz");
        app.handle(with(KeyCode::Char('a'), KeyModifiers::ALT));
        assert!(app.status.contains("not found"));
        // The Search menu lists it.
        assert!(app.menu_items(Menu::Search).iter().any(|i| i.action == Action::Replace));
    }

    #[test]
    fn check_before_run_builds_only_after_a_clean_check() {
        let _settings = isolate_settings();
        let files = temp_files(&["checked.rs"]);
        let mut app = App::new(&files);
        assert!(!app.check_before);
        assert!(matches!(app.act(Action::Run), Effect::Build { after: After::Run, .. }));
        app.act(Action::ToggleCheckBefore);
        assert!(app.check_before);
        assert_eq!(rust_ide::platform::read_setting("check_before").as_deref(), Some("true"));
        assert!(matches!(app.act(Action::Run), Effect::Check { .. }));
        let bad = Diagnostic { level: "error".into(), message: "oops".into(), file: None, line: 1, column: 1 };
        let failed = BuildResult { success: false, diagnostics: vec![bad], target: None };
        assert_eq!(app.check_finished(failed), Effect::None, "errors stop the run");
        assert!(matches!(app.act(Action::Compile), Effect::Check { .. }));
        let ok = BuildResult { success: true, diagnostics: vec![], target: None };
        assert!(matches!(app.check_finished(ok), Effect::Build { after: After::Compile, .. }), "a clean check goes on to compile");
        // A plain syntax check never builds.
        assert!(matches!(app.act(Action::SyntaxCheck), Effect::Check { .. }));
        let ok = BuildResult { success: true, diagnostics: vec![], target: None };
        assert_eq!(app.check_finished(ok), Effect::None);
        app.act(Action::ToggleCheckBefore);
        assert!(!app.check_before);
        assert!(app.menu_items(Menu::Options)[2].label.contains("[ ]"));
    }

    fn click(x: u16, y: u16) -> MouseEvent {
        MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: x, row: y, modifiers: KeyModifiers::NONE }
    }

    #[test]
    fn the_mouse_places_the_cursor_and_drives_menus_and_tabs() {
        let screen = Rect::new(0, 0, 80, 24);
        let mut app = App::new(&[]);
        typed(&mut app, "one
two words
three");
        // Row 0 of the screen is the menu bar, so the editor starts on row 1.
        app.mouse(click(4, 2), screen);
        assert_eq!((app.buffer().row, app.buffer().char_col()), (1, 4));
        app.mouse(click(70, 1), screen);
        assert_eq!((app.buffer().row, app.buffer().char_col()), (0, 3), "past the end of a line");
        // The File title opens its menu; a second click closes it.
        app.mouse(click(2, 0), screen);
        assert!(matches!(app.mode, Mode::Menu { menu: Menu::File, .. }));
        app.mouse(click(2, 0), screen);
        assert!(matches!(app.mode, Mode::Edit));
        // Open the Options menu by clicking its title, then click its item.
        let x = MENUS.iter().take_while(|&&m| m != Menu::Options).map(|m| m.title().len() as u16 + 3).sum::<u16>() + 2;
        app.mouse(click(x, 0), screen);
        assert!(matches!(app.mode, Mode::Menu { menu: Menu::Options, .. }));
        let _settings = isolate_settings();
        app.mouse(click(x + 2, 3), screen);
        assert!(matches!(app.mode, Mode::Edit), "a click on the separator line closes the menu");
        assert!(!app.check_before);
        app.mouse(click(x, 0), screen);
        app.mouse(click(x + 2, 5), screen);
        assert!(matches!(app.mode, Mode::Edit));
        assert!(app.check_before, "the clicked menu item ran");
        // Clicking outside an open menu just closes it.
        app.mouse(click(2, 0), screen);
        app.mouse(click(60, 15), screen);
        assert!(matches!(app.mode, Mode::Edit));
        // Tabs: with two windows the second row is the tab row.
        app.act(Action::New);
        app.handle(key(KeyCode::Char('r')));
        assert_eq!(app.windows.len(), 2);
        assert_eq!(app.active, 1);
        app.mouse(click(2, 1), screen);
        assert_eq!(app.active, 0, "clicking the first tab selects it");
        // The wheel moves the cursor.
        app.mouse(MouseEvent { kind: MouseEventKind::ScrollDown, column: 5, row: 5, modifiers: KeyModifiers::NONE }, screen);
        assert_eq!(app.buffer().row, 2);
    }

    #[test]
    fn line_numbers_toggle_shift_the_text_and_clicks() {
        let _settings = isolate_settings();
        let screen = Rect::new(0, 0, 80, 24);
        let mut app = App::new(&[]);
        typed(&mut app, "one
two
three");
        let before = crate::ui::editor_area(&app, screen);
        app.act(Action::ToggleLineNumbers);
        assert!(app.line_numbers);
        assert_eq!(rust_ide::platform::read_setting("line_numbers").as_deref(), Some("true"));
        let after = crate::ui::editor_area(&app, screen);
        assert_eq!(after.x, before.x + 2, "one digit and a space");
        // Clicking the third text column of line 1 lands on column 2 (the gutter is skipped).
        app.mouse(click(after.x + 2, 1), screen);
        assert_eq!((app.buffer().row, app.buffer().char_col()), (0, 2));
        app.mouse(click(0, 1), screen);
        assert_eq!(app.buffer().char_col(), 2, "a click in the numbers does nothing");
        assert!(app.menu_items(Menu::Options).iter().any(|i| i.label.starts_with("Line numbers") && i.label.contains("[x]")));
        app.act(Action::ToggleLineNumbers);
        assert_eq!(crate::ui::editor_area(&app, screen), before);
    }

    #[test]
    fn environment_items_open_the_files_and_the_listing() {
        let _settings = isolate_settings();
        let mut app = App::new(&[]);
        assert!(app.menu_items(Menu::Tools).iter().any(|i| i.label == "General environment..."));
        app.act(Action::GeneralEnv);
        let path = app.buffer().path.clone().expect("the general file is open");
        assert!(path.ends_with("environment.env"), "{}", path.display());
        assert!(app.buffer().text().contains("HTTP_PROXY"), "starts from the commented template");
        app.act(Action::ShowEnv);
        assert!(app.buffer().path.is_none());
        assert!(app.buffer().text().contains("# General:"));
        assert!(app.buffer().text().contains("# Project:"));
    }

    #[test]
    fn the_open_files_and_cursors_come_back_in_the_next_session() {
        let _settings = isolate_settings();
        let root = std::env::temp_dir().join("rust-ide-tui-session");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let (a, b) = (root.join("a.rs"), root.join("b.py"));
        std::fs::write(&a, "fn main() {}\nlet x = 1;\n").unwrap();
        std::fs::write(&b, "print(1)\nprint(\"héllo\")\n").unwrap();

        let mut app = App::new(&[a.clone(), b.clone()]);
        app.select(1);
        app.buffer_mut().go_to(1, 9);
        app.select(0);
        app.buffer_mut().go_to(1, 4);
        let snapshot = app.session_snapshot();
        assert_eq!((snapshot.files.len(), snapshot.active), (2, 0));
        assert_eq!((snapshot.files[0].line, snapshot.files[0].column, snapshot.files[1].column), (1, 4, 9));
        app.save_session();

        let mut next = App::new(&[]);
        let session = rust_ide::session::startup(&root).expect("the session was saved");
        next.restore_session(&session);
        assert_eq!(next.windows.iter().filter(|w| w.path.is_some()).count(), 2, "the blank window was reused");
        assert_eq!(next.buffer().path.as_deref(), Some(a.as_path()));
        assert_eq!((next.buffer().row, next.buffer().char_col()), (1, 4));
        assert!(next.status.contains("Restored"), "{}", next.status);
        next.select(1);
        assert_eq!((next.buffer().row, next.buffer().char_col()), (1, 9));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn menus_are_grouped_and_the_arrows_skip_the_lines() {
        let app = App::new(&[]);
        for (menu, groups) in [(Menu::Edit, 3), (Menu::Tools, 2), (Menu::Options, 3)] {
            let items = app.menu_items(menu);
            assert_eq!(items.split(|i| i.is_separator()).filter(|g| !g.is_empty()).count(), groups, "{menu:?}");
            assert!(!items[0].is_separator() && !items[items.len() - 1].is_separator());
            let mut hotkeys: Vec<char> = items.iter().filter(|i| !i.is_separator()).map(|i| i.hotkey).collect();
            let count = hotkeys.len();
            hotkeys.sort();
            hotkeys.dedup();
            assert_eq!(hotkeys.len(), count, "{menu:?} hotkeys are unique");
        }
        assert!(!app.menu_items(Menu::Options).iter().any(|i| i.label.contains("environment")), "moved to Tools");
        let mut app = App::new(&[]);
        app.handle(with(KeyCode::Char('t'), KeyModifiers::ALT));
        assert!(matches!(app.mode, Mode::Menu { menu: Menu::Tools, item: 0 }), "Alt+T opens Tools");
        for _ in 0..3 {
            app.handle(key(KeyCode::Down));
        }
        assert!(matches!(app.mode, Mode::Menu { item: 4, .. }), "Down jumps over the separator");
        app.handle(key(KeyCode::Up));
        assert!(matches!(app.mode, Mode::Menu { item: 2, .. }));
    }

    #[test]
    fn shift_arrows_select_and_the_clipboard_keys_work() {
        let mut app = App::new(&[]);
        typed(&mut app, "hello world");
        app.handle(key(KeyCode::Home));
        for _ in 0..5 {
            app.handle(with(KeyCode::Right, KeyModifiers::SHIFT));
        }
        assert_eq!(app.buffer().selected_text().as_deref(), Some("hello"));
        assert_eq!(app.handle(with(KeyCode::Char('c'), KeyModifiers::CONTROL)), Effect::Copy("hello".into()));
        assert_eq!(app.clipboard, "hello");
        assert_eq!(app.handle(with(KeyCode::Char('x'), KeyModifiers::CONTROL)), Effect::Copy("hello".into()));
        assert_eq!(app.buffer().text(), " world");
        assert_eq!(app.handle(with(KeyCode::Char('c'), KeyModifiers::CONTROL)), Effect::None, "nothing selected");
        assert!(app.status.contains("Select text first"));
        assert_eq!(app.handle(with(KeyCode::Char('v'), KeyModifiers::CONTROL)), Effect::Paste);
        app.paste("hi");
        assert_eq!(app.buffer().text(), "hi world");
        // Select all, then typing replaces everything.
        app.handle(with(KeyCode::Char('a'), KeyModifiers::CONTROL));
        typed(&mut app, "x");
        assert_eq!(app.buffer().text(), "x");
        // Backspace removes a selection; moving without Shift drops it.
        typed(&mut app, "yz");
        app.handle(with(KeyCode::Left, KeyModifiers::SHIFT));
        app.handle(key(KeyCode::Backspace));
        assert_eq!(app.buffer().text(), "xy");
        app.handle(with(KeyCode::Home, KeyModifiers::SHIFT));
        app.handle(key(KeyCode::Right));
        assert!(app.buffer().selection().is_none());
        // Text pasted by the terminal (bracketed paste) goes in as it is.
        app.paste_event("a\n    b");
        assert_eq!(app.buffer().text(), "xa\n    by");
    }

    #[test]
    fn the_mouse_drags_a_selection_and_right_click_opens_a_menu() {
        let screen = Rect::new(0, 0, 80, 24);
        let mut app = App::new(&[]);
        typed(&mut app, "one two\nthree");
        let event = |kind, x, y| MouseEvent { kind, column: x, row: y, modifiers: KeyModifiers::NONE };
        app.mouse(click(0, 1), screen);
        app.mouse(event(MouseEventKind::Drag(MouseButton::Left), 3, 1), screen);
        app.mouse(event(MouseEventKind::Up(MouseButton::Left), 3, 1), screen);
        assert_eq!(app.buffer().selected_text().as_deref(), Some("one"));
        // Right-click keeps the selection and opens the menu; C copies.
        app.mouse(event(MouseEventKind::Down(MouseButton::Right), 10, 2), screen);
        assert!(matches!(app.mode, Mode::Context { x: 10, y: 2, item: 0 }));
        assert_eq!(app.handle(key(KeyCode::Char('c'))), Effect::Copy("one".into()));
        assert!(matches!(app.mode, Mode::Edit));
        // Without a selection the click places the cursor; Esc closes; a click on an item runs it.
        app.mouse(click(2, 2), screen);
        app.mouse(event(MouseEventKind::Down(MouseButton::Right), 2, 2), screen);
        assert_eq!((app.buffer().row, app.buffer().char_col()), (1, 2));
        app.handle(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Edit));
        app.mouse(event(MouseEventKind::Down(MouseButton::Right), 2, 2), screen);
        app.handle(key(KeyCode::Up));
        assert!(matches!(app.mode, Mode::Context { item: 5, .. }), "Up wraps to the last item, over the separator");
        // Row 2 is the frame; Select all is the fourth item, on row 2 + 4.
        app.mouse(click(4, 6), screen);
        assert!(matches!(app.mode, Mode::Edit));
        assert_eq!(app.buffer().selected_text().as_deref(), Some("one two\nthree"));
        // Shift+click extends a selection.
        app.mouse(click(0, 1), screen);
        app.mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: 7, row: 1, modifiers: KeyModifiers::SHIFT }, screen);
        assert_eq!(app.buffer().selected_text().as_deref(), Some("one two"));
    }

    #[test]
    fn remove_double_empty_lines_from_the_edit_menu() {
        let mut app = App::new(&[]);
        typed(&mut app, "a\n\n\n\nb\n\nc");
        app.handle(with(KeyCode::Char('e'), KeyModifiers::ALT));
        app.handle(key(KeyCode::Char('d')));
        assert_eq!(app.buffer().text(), "a\n\nb\n\nc");
        assert!(app.status.starts_with("Removed 2 extra empty line(s)"), "{}", app.status);
        app.act(Action::RemoveEmptyLines);
        assert_eq!(app.status, "No double empty lines");
        app.act(Action::Undo);
        assert_eq!(app.buffer().text(), "a\n\n\n\nb\n\nc");
    }

    #[test]
    fn words_of_the_file_and_keywords_are_offered_while_typing() {
        let files = temp_files(&["wordlist.rs"]);
        let mut app = App::new(&files);
        app.handle(with(KeyCode::End, KeyModifiers::CONTROL));
        typed(&mut app, "let counter = 1;\nco");
        let list = app.completion.as_ref().expect("two letters open the list");
        let labels: Vec<&str> = list.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels[0], "counter", "{labels:?}");
        assert!(labels.contains(&"const") && labels.contains(&"continue"), "keywords follow: {labels:?}");
        typed(&mut app, "u");
        assert_eq!(app.visible_completions().len(), 1, "the list narrows");
        app.handle(key(KeyCode::Enter));
        assert!(app.completion.is_none());
        assert_eq!(app.buffer().lines[app.buffer().row], "counter");
        // Esc closes it; a space closes it; Backspace keeps it in step.
        typed(&mut app, " co");
        assert!(app.completion.is_some());
        app.handle(key(KeyCode::Esc));
        assert!(app.completion.is_none());
        typed(&mut app, "n");
        app.handle(key(KeyCode::Down));
        assert_eq!(app.completion.as_ref().unwrap().selected, 1, "Down moves in the list, not the text");
        app.handle(key(KeyCode::Backspace));
        assert!(app.completion.is_some());
        app.handle(key(KeyCode::Backspace));
        assert!(app.completion.is_none(), "one letter is not enough");
        app.handle(with(KeyCode::Char(' '), KeyModifiers::CONTROL));
        assert!(app.completion.is_some(), "Ctrl+Space asks with one letter");
        app.handle(key(KeyCode::Tab));
        assert!(app.buffer().lines[app.buffer().row].ends_with(" counter"), "Tab accepts too");
    }

    #[test]
    fn settings_dialog_changes_options_and_the_theme() {
        let _settings = isolate_settings();
        let mut app = App::new(&[]);
        app.act(Action::Settings);
        assert!(matches!(app.mode, Mode::Settings { selected: 0 }));
        app.handle(key(KeyCode::Char('n')));
        assert!(app.line_numbers);
        app.handle(key(KeyCode::Char('s')));
        assert!(app.two_line_status);
        assert_eq!(rust_ide::platform::read_setting("status_lines").as_deref(), Some("2"));
        assert_eq!(theme_label(), "Turbo");
        app.handle(key(KeyCode::Char('t')));
        assert_eq!((theme_label(), rust_ide::platform::read_setting("theme").as_deref()), ("Mono", Some("mono")));
        app.handle(key(KeyCode::Right));
        assert_eq!(theme_label(), "Modern");
        app.handle(key(KeyCode::Left));
        app.handle(key(KeyCode::Left));
        assert_eq!(theme_label(), "Turbo");
        assert!(matches!(app.mode, Mode::Settings { selected: 5 }), "the dialog stays open");
        assert_eq!(app.handle(key(KeyCode::Char('d'))), Effect::CheckDeps { startup: false }, "Dependencies opens from here");
        assert!(matches!(app.mode, Mode::Deps(_)));
        app.act(Action::ToggleLineNumbers);
        app.act(Action::ToggleStatusLines);
        set_theme("turbo");
    }

    #[test]
    fn the_dependencies_dialog_installs_copies_and_shows_steps() {
        use rust_ide::deps::{Dep, Need};
        let mut app = App::new(&[]);
        assert_eq!(app.act(Action::Dependencies), Effect::CheckDeps { startup: false });
        let dep = |name: &'static str, found: Option<&str>, auto: Option<Vec<String>>| Dep {
            name,
            purpose: "testing",
            need: Need::Required,
            found: found.map(str::to_string),
            install: format!("install {name}"),
            auto,
        };
        app.deps_checked(vec![dep("rustc", Some("rustc 1.90"), None), dep("gdb", None, Some(vec!["get".into(), "gdb".into()])), dep("cc", None, None)], false);
        let Mode::Deps(dialog) = &app.mode else { panic!() };
        assert_eq!(dialog.message, "2 required or recommended tool(s) missing");
        app.handle(key(KeyCode::Enter));
        let Mode::Deps(dialog) = &app.mode else { panic!() };
        assert_eq!(dialog.message, "rustc is installed");
        app.handle(key(KeyCode::Down));
        assert_eq!(app.handle(key(KeyCode::Char('i'))), Effect::InstallDep { name: "gdb", command: vec!["get".into(), "gdb".into()] });
        assert_eq!(app.dep_installed("gdb", Ok("done".into())), Effect::CheckDeps { startup: false }, "checks again after installing");
        let Mode::Deps(dialog) = &app.mode else { panic!() };
        assert_eq!(dialog.message, "Installed gdb. done");
        app.handle(key(KeyCode::Down));
        assert_eq!(app.handle(key(KeyCode::Enter)), Effect::None, "no automatic install");
        assert_eq!(app.handle(key(KeyCode::Char('c'))), Effect::Copy("install cc".into()));
        app.handle(key(KeyCode::Char('m')));
        assert!(matches!(app.mode, Mode::Edit));
        assert!(app.status.starts_with("Manual steps for cc"), "{}", app.status);
        assert_eq!(app.windows.len(), 2, "the steps open in a window of their own");
    }

    #[test]
    fn the_rust_toolchain_is_chosen_per_project_and_shown() {
        let _settings = isolate_settings();
        let root = std::env::temp_dir().join("rust-ide-tui-toolchain");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let mut app = App::new(&[]);
        assert!(app.menu_items(Menu::Tools).iter().any(|i| i.label == "Rust toolchain..."));
        app.buffer_mut().path = Some(root.join("main.rs"));
        assert_eq!(app.toolchain_label(), "Rust default ");

        // The typed name (the "Other name..." row): checked, and bare channels get the GNU host on Windows.
        app.submit(PromptKind::Toolchain, "bad name!");
        assert!(app.status.contains("not a toolchain name"), "{}", app.status);
        app.submit(PromptKind::Toolchain, " nightly ");
        let nightly = rust_ide::env::qualify_toolchain("nightly", cfg!(windows));
        assert_eq!(app.toolchain_label(), format!("Rust {nightly} "));
        assert!(std::fs::read_to_string(root.join(".rust-ide").join("env")).unwrap().contains(&format!("RUSTUP_TOOLCHAIN={nightly}")));

        // The picker: default, installed ones, then "Other name...".
        let (gnu, msvc) = ("stable-x86_64-pc-windows-gnu".to_string(), "nightly-x86_64-pc-windows-msvc".to_string());
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        app.open_toolchains(&[gnu.clone(), msvc.clone()]);
        assert!(matches!(&app.mode, Mode::Toolchains { choices, selected: 3 } if choices.len() == 4), "starts on the stored choice, which is not installed");
        app.key(key(KeyCode::Home));
        app.key(key(KeyCode::Down));
        app.key(key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::Edit));
        assert_eq!(app.toolchain_label(), format!("Rust {gnu} "));
        app.open_toolchains(&[gnu.clone(), msvc.clone()]);
        assert!(matches!(&app.mode, Mode::Toolchains { selected: 1, .. }), "starts on the current choice");
        app.key(key(KeyCode::Down));
        app.key(key(KeyCode::Enter));
        assert_eq!(app.toolchain_label(), format!("Rust {msvc} "));
        app.open_toolchains(&[gnu.clone()]);
        app.key(key(KeyCode::Char('o')));
        app.key(key(KeyCode::Enter));
        assert!(matches!(&app.mode, Mode::Prompt { kind: PromptKind::Toolchain, .. }), "Other name asks for a name");
        app.mode = Mode::Edit;
        app.open_toolchains(&[gnu.clone()]);
        app.key(key(KeyCode::Home));
        app.key(key(KeyCode::Enter));
        assert_eq!(app.toolchain_label(), "Rust default ");
        app.open_toolchains(&[gnu]);
        app.key(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Edit));
        app.buffer_mut().path = Some(root.join("a.py"));
        assert_eq!(app.toolchain_label(), "");
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// The features shared with the window version: Output window, debugger, language server,
/// Vim, Markdown preview, window styles and the search bar.
#[cfg(test)]
mod port_tests {
    use super::*;
    use crate::live::Background;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use rust_ide::output;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn with(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn typed(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle(if c == '\n' { key(KeyCode::Enter) } else { key(KeyCode::Char(c)) });
            app.sync();
        }
    }

    fn file(name: &str, text: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("tui-port-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    fn terminal(app: &App, width: u16, height: u16) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| crate::ui::draw(frame, app)).unwrap();
        terminal
    }

    fn screen(app: &App, width: u16, height: u16) -> String {
        let terminal = terminal(app, width, height);
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| (0..width).map(|x| buffer[(x, y)].symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn target(name: &str) -> RunTarget {
        RunTarget { program: name.into(), args: Vec::new(), cwd: ".".into(), gui: false, entry: None }
    }

    #[test]
    fn menus_match_the_window_version() {
        let app = App::new(&[]);
        let titles: Vec<&str> = MENUS.iter().map(|m| m.title()).collect();
        assert_eq!(titles, ["File", "Edit", "Search", "Run", "Compile", "Debug", "Tools", "Options", "Window", "Help"]);
        let labels = |menu| app.menu_items(menu).into_iter().map(|i| i.label).collect::<Vec<_>>();
        assert_eq!(labels(Menu::Run), ["Run", "Run in external terminal", "Program reset", "User screen"]);
        let debug = labels(Menu::Debug);
        for expected in ["Toggle breakpoint", "Debug run", "Trace into", "Step over", "Step out", "Go to cursor", "Add watch...", "Clear watches", "Clear breakpoints", "Program reset"] {
            assert!(debug.iter().any(|l| l == expected), "{expected} missing from {debug:?}");
        }
        let window = labels(Menu::Window);
        assert!(window.iter().any(|l| l.starts_with("Markdown preview")));
        assert!(window.iter().any(|l| l.starts_with("Horizontal tabs") && l.ends_with("(*)")));
        assert!(labels(Menu::Options).iter().any(|l| l.starts_with("Vim mode")));
        assert!(app.menu_items(Menu::Help).iter().any(|i| i.action == Action::OpenUrl(platform::INSTALL_GUIDE_URL)));
        assert!(app.menu_items(Menu::Help).iter().any(|i| i.action == Action::OpenUrl(platform::GITHUB_URL)));
        // Alt+D opens the Debug menu.
        let mut app = App::new(&[]);
        app.handle(with(KeyCode::Char('d'), KeyModifiers::ALT));
        assert!(matches!(app.mode, Mode::Menu { menu: Menu::Debug, .. }));
    }

    #[test]
    fn programs_run_in_the_output_window() {
        let mut app = App::new(&[]);
        assert_eq!(app.act(Action::UserScreen), Effect::None);
        assert!(app.status.contains("No program"), "{}", app.status);
        assert_eq!(app.start_run(target("prog")), Effect::StartRun { target: target("prog"), run_id: 1 });
        assert!(app.output.visible && app.output.running);
        app.background(Background::Output(1, output::Event::Output("Hello\nName? ".into())));
        app.background(Background::Output(0, output::Event::Output("from an old run".into())));
        let text = screen(&app, 80, 20);
        assert!(text.contains("Output - prog (running)") && text.contains("Hello"), "{text}");
        assert!(!text.contains("old run"));
        assert!(text.contains("Alt+F5/Esc Hide"));
        // Typing goes to the program's input line, not the editor.
        typed(&mut app, "Ann");
        assert_eq!(app.output.input, "Ann");
        assert_eq!(app.buffer().text(), "");
        app.handle(with(KeyCode::F(5), KeyModifiers::ALT));
        assert!(!app.output.visible, "Alt+F5 hides it");
        typed(&mut app, "x");
        assert_eq!(app.buffer().text(), "x");
        app.background(Background::Output(1, output::Event::Exited(Some(0))));
        assert_eq!(app.status, "Program finished, exit code 0");
        app.handle(with(KeyCode::F(5), KeyModifiers::ALT));
        assert!(app.output.visible && app.output.screen.contains("[Program finished, exit code 0]"));
        app.handle(key(KeyCode::Esc));
        assert!(!app.output.visible);
        app.handle(with(KeyCode::F(2), KeyModifiers::CONTROL));
        assert_eq!(app.status, "No program is running");
    }

    #[test]
    fn a_built_program_starts_by_the_chosen_way() {
        let mut app = App::new(&[]);
        let ok = |target| BuildResult { success: true, diagnostics: vec![], target: Some(target) };
        assert!(matches!(app.build_finished(ok(target("a")), After::Run), Effect::StartRun { .. }));
        assert_eq!(app.build_finished(ok(target("a")), After::Compile), Effect::None);
    }

    #[test]
    fn the_debugger_steps_and_shows_watches_locals_and_the_stack() {
        let path = file("debugged.rs", "fn main() {\n    let x = 5;\n    println!(\"{x}\");\n}\n");
        let mut app = App::new(&[path.clone()]);
        assert_eq!(app.handle(key(KeyCode::F(8))), Effect::Build { path: path.clone(), after: After::Debug(dap::Start::Entry) });
        assert_eq!(app.handle(with(KeyCode::F(5), KeyModifiers::SHIFT)), Effect::Build { path: path.clone(), after: After::Debug(dap::Start::Breakpoints) });
        app.buffer_mut().go_to(2, 0);
        assert_eq!(app.handle(key(KeyCode::F(4))), Effect::Build { path: path.clone(), after: After::Debug(dap::Start::ToLine(2)) });
        app.handle(with(KeyCode::F(8), KeyModifiers::CONTROL));
        assert!(app.breakpoints[&path].contains(&2), "Ctrl+F8 sets a breakpoint");
        app.handle(with(KeyCode::F(7), KeyModifiers::CONTROL));
        assert!(matches!(app.mode, Mode::Prompt { kind: PromptKind::Watch, .. }));
        if let Mode::Prompt { input, .. } = &mut app.mode {
            input.clear();
        }
        typed(&mut app, "x\n");
        assert_eq!(app.watches, [("x".to_string(), String::new())]);

        // A session (as the main loop starts it).
        app.debug = Some(crate::live::DebugSession::default());
        app.debug_run_id = 1;
        app.buffer_mut().go_to(0, 0);
        let frame = dap::Frame { name: "main".into(), file: Some(path.clone()), line: 2 };
        let locals = vec![dap::Variable { name: "x".into(), value: "5".into() }];
        app.background(Background::Debug(1, dap::Event::Stopped { frames: vec![frame], locals }));
        assert_eq!((app.buffer().row, app.buffer().char_col()), (1, 4), "goes to the stop line, at its indent");
        assert_eq!(app.exec_line(), Some(1));
        app.background(Background::Debug(1, dap::Event::Evaluated { expression: "x".into(), value: "5".into() }));
        let text = screen(&app, 100, 20);
        for expected in ["Watches", "x = 5", "Locals", "Call stack", "main line 2"] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        assert_eq!(app.handle(key(KeyCode::F(5))), Effect::None, "F5 continues instead of building");
        app.background(Background::Debug(1, dap::Event::Ended));
        assert!(app.debug.is_none());
        app.act(Action::ClearBreakpoints);
        assert!(app.breakpoints.is_empty());
        app.act(Action::ClearWatches);
        assert!(!screen(&app, 100, 20).contains("Call stack"), "the panel goes when nothing is left");
    }

    #[test]
    fn live_errors_are_underlined_listed_and_stepped_through() {
        let path = file("live.rs", "fn main() {\n    let bad = 1;\n}\n");
        let mut app = App::new(&[path.clone()]);
        app.sync();
        let problem = lsp::Diagnostic { line: 1, start: 8, end_line: 1, end: 11, error: true, message: "unused variable".into() };
        app.background(Background::Lsp(lsp::Event::Diagnostics { path: path.clone(), items: vec![problem] }));
        assert_eq!(app.live.len(), 1);
        let items = app.panel_items();
        assert_eq!((items[0].line, items[0].column, items[0].level.as_str()), (2, 9, "error"));
        app.handle(with(KeyCode::F(8), KeyModifiers::ALT));
        assert_eq!((app.buffer().row, app.buffer().char_col()), (1, 8), "Alt+F8 goes to it");
        let terminal = terminal(&app, 80, 20);
        let cell = &terminal.backend().buffer()[(9, 2)];
        assert!(cell.modifier.contains(ratatui::style::Modifier::UNDERLINED), "the error is underlined");
        let text = screen(&app, 100, 20);
        assert!(text.contains("Error: unused variable"), "the status shows the cursor line's problem\n{text}");
        assert!(text.contains("Messages"));
        app.background(Background::Lsp(lsp::Event::Status("rust-analyzer: indexing".into())));
        assert!(screen(&app, 160, 20).contains("rust-analyzer: indexing"));
    }

    #[test]
    fn the_language_server_completes_after_a_dot() {
        let path = file("complete.rs", "fn main() {\n}\n");
        let mut app = App::new(&[path.clone()]);
        let (tx, mut rx) = iced::futures::channel::mpsc::unbounded();
        app.background(Background::Lsp(lsp::Event::Ready(tx)));
        assert!(matches!(rx.try_recv(), Ok(lsp::Request::Open { .. })), "the file is opened on the server");
        app.buffer_mut().go_to(1, 0);
        typed(&mut app, "v.");
        let mut asked = None;
        while let Ok(request) = rx.try_recv() {
            if let lsp::Request::Complete { id, trigger, .. } = request {
                asked = Some((id, trigger));
            }
        }
        let (id, trigger) = asked.expect("'.' asks the server");
        assert_eq!(trigger, Some('.'));
        assert!(app.completion.as_ref().is_some_and(|c| c.waiting));
        let item = |label: &str| lsp::CompletionItem {
            label: label.into(),
            insert: label.into(),
            detail: String::new(),
            kind: "method",
            filter: label.into(),
        };
        app.background(Background::Lsp(lsp::Event::Completions { id, items: vec![item("len"), item("push")] }));
        assert_eq!(app.visible_completions().len(), 2);
        typed(&mut app, "p");
        assert_eq!(app.visible_completions().len(), 1, "the typed letters narrow the list");
        app.handle(key(KeyCode::Enter));
        assert_eq!(app.buffer().lines[1], "v.push}");
        // Saving tells the server.
        app.save();
        let mut saved = false;
        while let Ok(request) = rx.try_recv() {
            saved |= matches!(request, lsp::Request::Save { .. });
        }
        assert!(saved);
    }

    #[test]
    fn vim_mode_edits_with_vim_keys() {
        let _settings = super::tests::isolate_settings();
        let mut app = App::new(&[]);
        app.act(Action::ToggleVim);
        assert_eq!(platform::read_setting("vim").as_deref(), Some("true"));
        assert_eq!(app.vim.as_ref().unwrap().mode, vim::Mode::Normal);
        typed(&mut app, "ione\ntwo");
        app.handle(key(KeyCode::Esc));
        assert_eq!(app.buffer().text(), "one\ntwo");
        assert_eq!(app.vim.as_ref().unwrap().mode, vim::Mode::Normal);
        typed(&mut app, "dd");
        assert_eq!(app.buffer().text(), "one");
        typed(&mut app, "u");
        assert_eq!(app.buffer().text(), "one\ntwo");
        app.handle(with(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert_eq!(app.buffer().text(), "one", "Ctrl+R is Vim's redo, not Replace");
        assert!(app.search.is_none());
        typed(&mut app, ":q\n");
        assert!(app.status.contains("No write since last change"), "{}", app.status);
        assert!(screen(&app, 80, 10).contains("NORMAL"));
        typed(&mut app, ":q!");
        assert_eq!(app.handle(key(KeyCode::Enter)), Effect::Quit);
        app.act(Action::ToggleVim);
        assert!(app.vim.is_none());
        assert_eq!(app.status, "Vim mode off");
    }

    #[test]
    fn markdown_preview_beside_the_text_or_in_a_tab() {
        let _settings = super::tests::isolate_settings();
        let path = file("readme.md", "# Title\n\nSome *styled* text.\n");
        let rust = file("plain.rs", "fn main() {}\n");
        let mut app = App::new(&[rust, path.clone()]);
        app.select(0);
        app.handle(with(KeyCode::F(6), KeyModifiers::ALT));
        assert_eq!(app.status, "The preview is for Markdown (.md) files");
        app.select(1);
        app.handle(with(KeyCode::F(6), KeyModifiers::ALT));
        assert!(app.preview_visible());
        let text = screen(&app, 80, 12);
        let line = text.lines().find(|l| l.contains("Some styled text.")).expect(&text);
        assert!(line.contains("Some *styled* text."), "text and preview side by side: {line}");

        app.act(Action::CyclePreviewMode);
        assert_eq!(app.status, "Markdown preview opens a new tab");
        assert_eq!(platform::read_setting("preview_mode").as_deref(), Some("tab"));
        app.handle(with(KeyCode::F(6), KeyModifiers::ALT));
        assert_eq!(app.windows.len(), 3);
        assert_eq!(app.buffer().name(), "Preview readme.md");
        assert!(screen(&app, 80, 12).contains("Title"));
        typed(&mut app, "x");
        assert!(app.status.contains("cannot be edited"));
        assert_eq!(app.windows[1].text(), "# Title\n\nSome *styled* text.\n");
        app.handle(with(KeyCode::F(6), KeyModifiers::ALT));
        assert_eq!(app.windows.len(), 2, "Alt+F6 on the tab closes it");
        assert_eq!(app.buffer().path.as_deref(), Some(path.as_path()));
    }

    #[test]
    fn window_styles_legacy_horizontal_and_vertical() {
        let _settings = super::tests::isolate_settings();
        let a = file("style_a.rs", "a\n");
        let b = file("style_b.rs", "b\n");
        let mut app = App::new(&[a, b]);
        assert_eq!(app.window_style, WindowStyle::Horizontal);
        assert!(screen(&app, 80, 10).lines().nth(1).unwrap().contains("2 style_b.rs"), "a row of tabs");
        app.act(Action::SetWindowStyle(WindowStyle::Legacy));
        assert_eq!(platform::read_setting("window_style").as_deref(), Some("legacy"));
        let text = screen(&app, 80, 10);
        let title = text.lines().nth(1).unwrap();
        assert!(title.contains("[\u{25A0}]") && title.contains("style_a.rs") && title.contains("1/2"), "{title}");
        app.act(Action::SetWindowStyle(WindowStyle::Vertical));
        assert_eq!(app.status, "Window style: vertical tabs");
        let text = screen(&app, 80, 10);
        assert!(text.lines().nth(1).unwrap().starts_with(" 1 style_a.rs"), "{text}");
        assert!(text.lines().nth(2).unwrap().starts_with(" 2 style_b.rs"), "{text}");
        let click = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: 3, row: 1, modifiers: KeyModifiers::NONE };
        app.mouse(click, Rect::new(0, 0, 80, 10));
        assert_eq!(app.active, 0, "a click in the list picks the window");
    }

    #[test]
    fn settings_rows_and_the_startup_check() {
        let _settings = super::tests::isolate_settings();
        let mut app = App::new(&[]);
        app.act(Action::Settings);
        app.handle(key(KeyCode::Char('v')));
        assert!(app.vim.is_some());
        app.handle(key(KeyCode::Char('w')));
        assert_eq!(app.window_style, WindowStyle::Vertical);
        app.handle(key(KeyCode::Char('p')));
        assert_eq!(app.preview_mode, PreviewMode::Tab);
        let text = screen(&app, 90, 24);
        for expected in ["Markdown preview opens", "a new tab", "Window style", "Vertical tabs", "Vim mode"] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        app.handle(key(KeyCode::Char('v')));
        app.handle(key(KeyCode::Esc));

        use rust_ide::deps::{Dep, Need};
        let dep = |found: Option<&str>| Dep {
            name: "gdb",
            purpose: "debugger",
            need: Need::Recommended,
            found: found.map(str::to_string),
            install: "install gdb".into(),
            auto: None,
        };
        app.deps_checked(vec![dep(Some("gdb 15"))], true);
        assert!(matches!(app.mode, Mode::Edit), "nothing missing: no dialog at startup");
        app.output.visible = true;
        app.deps_checked(vec![dep(None)], true);
        assert!(matches!(app.mode, Mode::Edit), "not over the Output window");
        app.output.visible = false;
        app.deps_checked(vec![dep(None)], true);
        assert!(matches!(app.mode, Mode::Deps(_)), "something missing: the dialog opens");
        app.handle(key(KeyCode::Char('s')));
        assert!(!app.check_at_startup);
        assert_eq!(platform::read_setting("check_dependencies").as_deref(), Some("false"));
    }
}
