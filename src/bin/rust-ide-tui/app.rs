//! State and keyboard handling of the terminal IDE. Work that needs the
//! real terminal (building, running a program, the user screen) is returned
//! as an [`Effect`] for the main loop to carry out.

use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use rust_ide::runner::{BuildResult, Diagnostic, RunTarget};
use rust_ide::filetype;
use rust_ide::search;
use rust_ide::templates::{self, Lang, Size};

use crate::buffer::Buffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    File,
    Edit,
    Search,
    Run,
    Compile,
    Options,
    Window,
    Help,
}

pub const MENUS: [Menu; 8] =
    [Menu::File, Menu::Edit, Menu::Search, Menu::Run, Menu::Compile, Menu::Options, Menu::Window, Menu::Help];

impl Menu {
    pub fn title(self) -> &'static str {
        match self {
            Menu::File => "File",
            Menu::Edit => "Edit",
            Menu::Search => "Search",
            Menu::Run => "Run",
            Menu::Compile => "Compile",
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
    Find,
    FindNext,
    Replace,
    GoToLine,
    ToggleCheckBefore,
    ToggleLineNumbers,
    Run,
    UserScreen,
    Compile,
    SyntaxCheck,
    NextMessage,
    PrevMessage,
    NextWindow,
    PrevWindow,
    CloseWindow,
    SelectWindow(usize),
    Help,
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

/// What the main loop must do with the terminal.
#[derive(Debug, PartialEq)]
pub enum Effect {
    None,
    /// Build (and run afterwards when `run` is true); the status says "Compiling...".
    Build { path: PathBuf, run: bool },
    /// Look for errors only: no program is built or run.
    Check { path: PathBuf },
    /// Run a built program in the terminal (Turbo Pascal's user screen).
    Run(RunTarget),
    /// Show the terminal's normal screen until a key is pressed.
    UserScreen,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    SaveAs,
    Find,
    GoTo,
    /// Replace, step 1: the text to look for.
    ReplaceFind,
    /// Replace, step 2: what to put instead.
    ReplaceWith,
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
}

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
    /// The text being replaced, between the two Replace prompts.
    replace_query: String,
    /// Options > Check before run: F5 / F9 look for errors first.
    pub check_before: bool,
    /// Options > Line numbers: a column of line numbers left of the text.
    pub line_numbers: bool,
    /// After a check that finds no errors: build (and run when true).
    then_build: Option<bool>,
    /// A pending "press again to discard" for this action.
    confirm: Option<Action>,
    /// Program shown on the user screen (for its title).
    pub last_run: Option<String>,
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
            replace_query: String::new(),
            check_before: false,
            line_numbers: false,
            then_build: None,
            confirm: None,
            last_run: None,
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
            Menu::Edit => vec![item("Undo", 'u', "Ctrl+Z", Action::Undo), item("Redo", 'r', "Ctrl+Y", Action::Redo)],
            Menu::Search => vec![
                item("Find...", 'f', "Ctrl+F", Action::Find),
                item("Replace...", 'r', "Ctrl+H", Action::Replace),
                item("Search again", 's', "Ctrl+L", Action::FindNext),
                item("Go to line number...", 'g', "Ctrl+G", Action::GoToLine),
            ],
            Menu::Run => vec![item("Run", 'r', "F5", Action::Run), item("User screen", 'u', "Alt+F5", Action::UserScreen)],
            Menu::Compile => vec![
                item("Compile", 'c', "F9", Action::Compile),
                item("Syntax check", 's', "Alt+F9", Action::SyntaxCheck),
                item("Next message", 'n', "Alt+F8", Action::NextMessage),
                item("Previous message", 'p', "Alt+F7", Action::PrevMessage),
            ],
            Menu::Options => vec![
                item(
                    if self.check_before { "Check before run   [x]" } else { "Check before run   [ ]" },
                    'c',
                    "",
                    Action::ToggleCheckBefore,
                ),
                item(
                    if self.line_numbers { "Line numbers       [x]" } else { "Line numbers       [ ]" },
                    'l',
                    "",
                    Action::ToggleLineNumbers,
                ),
            ],
            Menu::Window => {
                let mut items = vec![
                    item("Next", 'n', "F6", Action::NextWindow),
                    item("Previous", 'p', "Shift+F6", Action::PrevWindow),
                    item("Close", 'c', "Alt+F3", Action::CloseWindow),
                ];
                for (index, window) in self.windows.iter().enumerate().take(9) {
                    let mark = if index == self.active { '>' } else { ' ' };
                    let star = if window.modified { "*" } else { "" };
                    let hotkey = char::from_digit(index as u32 + 1, 10).unwrap_or(' ');
                    items.push(item(format!("{}{mark}{}{star}", index + 1, window.name()), hotkey, "", Action::SelectWindow(index)));
                }
                items
            }
            Menu::Help => vec![item("Keyboard shortcuts", 'k', "F1", Action::Help)],
        }
    }

    // ----- Keys -----

    pub fn key(&mut self, key: KeyEvent) -> Effect {
        // Global keys work in every mode.
        if let Some(action) = global_key(&key) {
            self.mode = Mode::Edit;
            return self.act(action);
        }
        match &mut self.mode {
            Mode::Edit => self.edit_key(key),
            Mode::Menu { .. } => self.menu_key(key),
            Mode::Prompt { .. } => self.prompt_key(key),
            Mode::Open(_) => self.open_key(key),
            Mode::Language { .. } => self.language_key(key),
            Mode::Project(_) => self.project_key(key),
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
        if !matches!(key.code, KeyCode::F(_)) {
            self.confirm = None;
        }
        let b = self.buffer_mut();
        match key.code {
            KeyCode::Char(c) if !ctrl && !alt || ctrl && alt => b.insert_char(c),
            KeyCode::Enter => b.newline(),
            KeyCode::Backspace => b.backspace(),
            KeyCode::Delete => b.delete(),
            KeyCode::Tab => b.indent(),
            KeyCode::BackTab => b.unindent(),
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
        Effect::None
    }

    fn menu_key(&mut self, key: KeyEvent) -> Effect {
        let Mode::Menu { menu, item } = self.mode else { return Effect::None };
        let items = self.menu_items(menu);
        let step = |by: isize| MENUS[((MENUS.len() as isize + menu.index() as isize + by) as usize) % MENUS.len()];
        match key.code {
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Left => self.mode = Mode::Menu { menu: step(-1), item: 0 },
            KeyCode::Right => self.mode = Mode::Menu { menu: step(1), item: 0 },
            KeyCode::Up => self.mode = Mode::Menu { menu, item: (item + items.len() - 1) % items.len() },
            KeyCode::Down => self.mode = Mode::Menu { menu, item: (item + 1) % items.len() },
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
        let confirmed = self.confirm.take() == Some(action.clone());
        match action {
            Action::New => self.ask_language(false),
            Action::NewProject => self.ask_project(),
            Action::Open => {
                let dir = self
                    .buffer()
                    .path
                    .as_deref()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
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
                if unsaved == 0 || confirmed {
                    return Effect::Quit;
                }
                self.status = format!("{unsaved} window(s) have unsaved changes! F2 saves, Alt+X again quits.");
                self.confirm = Some(Action::Exit);
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
            Action::Find => self.mode = Mode::Prompt { kind: PromptKind::Find, input: self.last_query.clone() },
            Action::FindNext => self.find_next(),
            Action::Replace => self.mode = Mode::Prompt { kind: PromptKind::ReplaceFind, input: self.last_query.clone() },
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
            Action::Run | Action::Compile => {
                let run = action == Action::Run;
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
                    self.then_build = Some(run);
                    self.status = "Checking...".into();
                    return Effect::Check { path };
                }
                self.status = "Compiling...".into();
                return Effect::Build { path, run };
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
            Action::UserScreen => return Effect::UserScreen,
            Action::NextMessage => self.step_message(true),
            Action::PrevMessage => self.step_message(false),
            Action::NextWindow => self.select((self.active + 1) % self.windows.len()),
            Action::PrevWindow => self.select((self.active + self.windows.len() - 1) % self.windows.len()),
            Action::SelectWindow(index) => self.select(index),
            Action::CloseWindow => {
                if self.buffer().modified && !confirmed {
                    self.status = "Unsaved changes! F2 saves, Alt+F3 again closes.".into();
                    self.confirm = Some(Action::CloseWindow);
                } else {
                    self.close_window();
                }
            }
            Action::Help => self.mode = Mode::Help { scroll: 0 },
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
            PromptKind::Find => {
                self.last_query = input.to_string();
                self.find_next();
            }
            PromptKind::ReplaceFind => {
                if input.is_empty() {
                    self.status = "Nothing to search for".into();
                } else {
                    self.last_query = input.to_string();
                    self.replace_query = input.to_string();
                    self.mode = Mode::Prompt { kind: PromptKind::ReplaceWith, input: String::new() };
                }
            }
            PromptKind::ReplaceWith => {
                let query = std::mem::take(&mut self.replace_query);
                let count = self.buffer_mut().replace_all(&query, input);
                self.status = if count == 0 {
                    format!("Search string not found: {query}")
                } else {
                    format!("Replaced {count} occurrence(s) (Ctrl+Z undoes)")
                };
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
        self.status = match self.buffer_mut().save() {
            Ok(()) => format!("Saved {}", self.buffer().name()),
            Err(e) => format!("Cannot save: {e}"),
        };
    }

    fn find_next(&mut self) {
        if self.last_query.is_empty() {
            self.status = "Nothing to search for (Ctrl+F)".into();
            return;
        }
        let text = self.buffer().text();
        let from = self.buffer().offset() + 1;
        match search::find(&text, &self.last_query, from.min(text.len()), false) {
            Some((start, _, wrapped)) => {
                self.buffer_mut().go_to_offset(start);
                let count = search::all(&text, &self.last_query, false).len();
                self.status = format!("Found \"{}\" ({count} in file){}", self.last_query, if wrapped { ", wrapped" } else { "" });
            }
            None => self.status = format!("Search string not found: {}", self.last_query),
        }
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
    pub fn build_finished(&mut self, result: BuildResult, run: bool) -> Effect {
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
        match result.target {
            Some(target) if run => {
                self.last_run = Some(self.buffer().name());
                Effect::Run(target)
            }
            _ => Effect::None,
        }
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
            Some(run) if result.success => {
                let path = self.buffer().path.clone().unwrap_or_default();
                self.status.push_str("; compiling...");
                Effect::Build { path, run }
            }
            _ => Effect::None,
        }
    }

    // ----- Mouse -----

    /// A mouse event on a screen of the given size: menus, tabs and the editor.
    pub fn mouse(&mut self, event: MouseEvent, screen: Rect) -> Effect {
        let editor = crate::ui::editor_area(self, screen);
        let (x, y) = (event.column, event.row);
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
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
                if self.windows.len() > 1 && y == screen.y + 1 {
                    if let Some(index) = crate::ui::tab_at(self, x) {
                        self.select(index);
                    }
                } else if editor.contains(Position::new(x, y)) {
                    let (row, col) = (y - editor.y, x - editor.x);
                    let b = self.buffer_mut();
                    let (row, col) = (b.scroll + row as usize, b.hscroll + col as usize);
                    b.go_to(row, col);
                }
            }
            MouseEventKind::ScrollUp if matches!(self.mode, Mode::Edit) => self.buffer_mut().vertical(-3),
            MouseEventKind::ScrollDown if matches!(self.mode, Mode::Edit) => self.buffer_mut().vertical(3),
            _ => {}
        }
        Effect::None
    }

    fn step_message(&mut self, forward: bool) {
        if self.messages.is_empty() {
            self.status = "No messages".into();
            return;
        }
        let count = self.messages.len();
        let index = match self.message_index {
            Some(i) if forward => (i + 1) % count,
            Some(i) => (i + count - 1) % count,
            None if forward => 0,
            None => count - 1,
        };
        self.message_index = Some(index);
        let message = self.messages[index].clone();
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
        KeyCode::F(2) => Action::Save,
        KeyCode::F(3) if alt => Action::CloseWindow,
        KeyCode::F(3) => Action::Open,
        KeyCode::F(5) if alt => Action::UserScreen,
        KeyCode::F(5) => Action::Run,
        KeyCode::F(6) if shift => Action::PrevWindow,
        KeyCode::F(6) => Action::NextWindow,
        KeyCode::F(7) if alt => Action::PrevMessage,
        KeyCode::F(8) if alt => Action::NextMessage,
        KeyCode::F(9) if ctrl => Action::Run,
        KeyCode::F(9) if alt => Action::SyntaxCheck,
        KeyCode::F(9) => Action::Compile,
        KeyCode::Char(c) if ctrl && !alt => match c.to_ascii_lowercase() {
            's' => Action::Save,
            'o' => Action::Open,
            'z' => Action::Undo,
            'y' => Action::Redo,
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
mod tests {
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
        typed(&mut app, "alpha\n");
        assert_eq!((app.buffer().row, app.buffer().char_col()), (1, 5));
        app.handle(with(KeyCode::Char('l'), KeyModifiers::CONTROL));
        assert_eq!((app.buffer().row, app.buffer().char_col()), (0, 0), "wraps around");
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
        assert_eq!(app.windows.len(), 2, "unsaved: the first Alt+F3 warns");
        app.handle(with(KeyCode::F(3), KeyModifiers::ALT));
        assert_eq!(app.windows.len(), 1);
        app.open(&files[0]);
        assert_eq!(app.windows.len(), 1, "an open file is not opened twice");
    }

    #[test]
    fn exit_asks_twice_with_unsaved_changes() {
        let mut app = App::new(&[]);
        typed(&mut app, "x");
        assert_eq!(app.handle(with(KeyCode::Char('x'), KeyModifiers::ALT)), Effect::None);
        assert_eq!(app.handle(with(KeyCode::Char('x'), KeyModifiers::ALT)), Effect::Quit);
    }

    #[test]
    fn compile_needs_a_file_and_reports_errors() {
        let mut app = App::new(&[]);
        assert_eq!(app.handle(key(KeyCode::F(9))), Effect::None);
        assert!(matches!(app.mode, Mode::Prompt { kind: PromptKind::SaveAs, .. }) || app.status.contains("Save"));

        let files = temp_files(&["broken.rs"]);
        let mut app = App::new(&files);
        let effect = app.handle(key(KeyCode::F(9)));
        assert_eq!(effect, Effect::Build { path: files[0].clone(), run: false });
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
        assert_eq!(app.build_finished(result, false), Effect::None);
        assert_eq!((app.buffer().row, app.buffer().char_col()), (1, 3), "jumps to the first error");
        assert!(app.status.starts_with("ERROR 1/1"));
    }

    #[test]
    fn alt_f9_is_the_syntax_check_and_f9_still_compiles() {
        let files = temp_files(&["check_me.rs"]);
        let mut app = App::new(&files);
        assert_eq!(app.handle(with(KeyCode::F(9), KeyModifiers::ALT)), Effect::Check { path: files[0].clone() });
        assert_eq!(app.handle(key(KeyCode::F(9))), Effect::Build { path: files[0].clone(), run: false });
        assert_eq!(
            app.handle(with(KeyCode::F(9), KeyModifiers::CONTROL)),
            Effect::Build { path: files[0].clone(), run: true }
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
    fn isolate_settings() -> std::sync::MutexGuard<'static, ()> {
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
        app.handle(key(KeyCode::Char('3')));
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
        assert!(matches!(app.mode, Mode::Language { selected: 3, .. }));
        app.handle(key(KeyCode::Enter));
        assert_eq!(app.buffer().ext, Some("md"));
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
    fn replace_all_asks_twice_and_undoes() {
        let mut app = App::new(&[]);
        typed(&mut app, "Foo bar foo
FOO");
        app.handle(with(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(matches!(app.mode, Mode::Prompt { kind: PromptKind::ReplaceFind, .. }));
        typed(&mut app, "foo
");
        assert!(matches!(app.mode, Mode::Prompt { kind: PromptKind::ReplaceWith, .. }));
        typed(&mut app, "x
");
        assert!(matches!(app.mode, Mode::Edit));
        assert_eq!(app.buffer().text(), "x bar x
x", "case is ignored, like Find");
        assert!(app.status.contains("Replaced 3"));
        app.handle(with(KeyCode::Char('z'), KeyModifiers::CONTROL));
        assert_eq!(app.buffer().text(), "Foo bar foo
FOO", "one undo restores everything");
        // Nothing found.
        app.handle(with(KeyCode::Char('r'), KeyModifiers::CONTROL));
        typed(&mut app, "zzz

");
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
        assert!(matches!(app.act(Action::Run), Effect::Build { run: true, .. }));
        app.act(Action::ToggleCheckBefore);
        assert!(app.check_before);
        assert_eq!(rust_ide::platform::read_setting("check_before").as_deref(), Some("true"));
        assert!(matches!(app.act(Action::Run), Effect::Check { .. }));
        let bad = Diagnostic { level: "error".into(), message: "oops".into(), file: None, line: 1, column: 1 };
        let failed = BuildResult { success: false, diagnostics: vec![bad], target: None };
        assert_eq!(app.check_finished(failed), Effect::None, "errors stop the run");
        assert!(matches!(app.act(Action::Compile), Effect::Check { .. }));
        let ok = BuildResult { success: true, diagnostics: vec![], target: None };
        assert!(matches!(app.check_finished(ok), Effect::Build { run: false, .. }), "a clean check goes on to compile");
        // A plain syntax check never builds.
        assert!(matches!(app.act(Action::SyntaxCheck), Effect::Check { .. }));
        let ok = BuildResult { success: true, diagnostics: vec![], target: None };
        assert_eq!(app.check_finished(ok), Effect::None);
        app.act(Action::ToggleCheckBefore);
        assert!(!app.check_before);
        assert!(app.menu_items(Menu::Options)[0].label.contains("[ ]"));
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
        app.mouse(click(x + 2, 2), screen);
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
        assert!(app.menu_items(Menu::Options)[1].label.contains("[x]"));
        app.act(Action::ToggleLineNumbers);
        assert_eq!(crate::ui::editor_area(&app, screen), before);
    }
}
