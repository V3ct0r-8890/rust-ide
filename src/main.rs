//! rust-ide: a Turbo Pascalâ€“style IDE for Rust and Python (iced GUI front end).

// Release builds are GUI apps on Windows: no console window when started.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cursor;

use rust_ide::{dap, deps, highlight, lsp, output, platform, runner, search, vim};

use std::collections::{BTreeMap, BTreeSet};
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
const HELP_SCROLL: &str = "help-scroll";

/// Every keyboard shortcut, shown by F1. Grouped as (group, keys, action).
const SHORTCUTS: &[(&str, &str, &str)] = &[
    ("Menus", "F10", "Open the menu bar (File menu)"),
    ("Menus", "Alt+F E S R C D O W H", "Open File, Edit, Search, Run, Compile, Debug, Options, Window, Help"),
    ("Menus", "Left / Right", "Previous / next menu"),
    ("Menus", "Up / Down, Enter", "Choose a menu item"),
    ("Menus", "Red letter", "Run that menu item"),
    ("Menus", "Esc", "Close the menu, dialog or panel"),
    ("File", "F2 / Ctrl+S", "Save"),
    ("File", "F3 / Ctrl+O", "Open (in a new window)"),
    ("File", "Alt+F N / A", "New window / Save as (File menu)"),
    ("File", "Alt+X", "Exit (press twice to discard changes)"),
    ("Window", "F6 / Shift+F6", "Next / previous window"),
    ("Window", "Alt+1 ... Alt+9", "Go to window 1-9"),
    ("Window", "Alt+0", "Window list (Window menu)"),
    ("Window", "Alt+F3", "Close the window (press twice to discard changes)"),
    ("Edit", "Ctrl+Z / Ctrl+Y", "Undo / redo (Ctrl+Shift+Z also redoes)"),
    ("Edit", "Tab / Shift+Tab", "Indent / unindent"),
    ("Edit", "Ctrl+Space", "Code completion (also after . and ::)"),
    ("Edit", "Up / Down, Enter, Esc", "In the completion list: choose, accept, close"),
    ("Search", "Ctrl+F", "Find"),
    ("Search", "Ctrl+H", "Replace"),
    ("Search", "Ctrl+L", "Search again"),
    ("Search", "Ctrl+G", "Go to line number"),
    ("Search", "Enter", "In the Find box: next match (Previous is a button)"),
    ("Search", "Tab", "Next field in the search bar"),
    ("Search", "Alt+M", "Toggle match case"),
    ("Compile", "F9", "Compile the current file (checks the syntax first when that option is on)"),
    ("Compile", "Alt+F9", "Syntax check: find errors without building or running"),
    ("Compile", "Alt+F8 / Alt+F7", "Next / previous error or warning"),
    ("Run", "F5 / Ctrl+F9", "Save, compile and run"),
    ("Run", "Alt+F5", "Show / hide the Output window (User screen)"),
    ("Run", "Ctrl+F2", "Program reset: stop the running program"),
    ("Run", "Enter", "In the Output window: send the typed input"),
    ("Options", "Alt+O D", "Dependencies: Up/Down, I install, C copy, G guide, R re-check, S startup check"),
    ("Options", "Alt+O V", "Vim mode on / off"),
    ("Options", "Alt+O C", "Check the syntax before compile / run on / off"),
    ("Help", "F1", "This list (Up/Down, Page Up/Down scroll; Esc or F1 closes)"),
    ("Vim", "i a o O, Esc", "Insert mode, back to Normal"),
    ("Vim", "h j k l w b e 0 ^ $ gg G", "Motions (with counts)"),
    ("Vim", "d c y > < + motion", "Operators; dd yy cc >> << for lines"),
    ("Vim", "x X D C Y p P J r u Ctrl+R", "Editing"),
    ("Vim", "v, /text, n N", "Visual mode, search"),
    ("Vim", ":w :q :q! :wq :<line>", "Commands"),
    ("Debug", "Ctrl+F8", "Toggle breakpoint on the cursor line"),
    ("Debug", "F7 / F8", "Trace into / step over (starts debugging at the first line)"),
    ("Debug", "Shift+F8", "Step out of the current function"),
    ("Debug", "F4", "Go to cursor: run to the cursor line"),
    ("Debug", "Shift+F5", "Debug run: run to the next breakpoint"),
    ("Debug", "F5 / Ctrl+F9", "While debugging: continue"),
    ("Debug", "Ctrl+F7", "Add a watch expression"),
    ("Debug", "Ctrl+F2", "Program reset: stop debugging"),
];

const USAGE: &str = "\
Usage: rust-ide [OPTIONS] [FILE...]

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
    Debug,
    Options,
    Window,
    Help,
}

const MENUS: [Menu; 9] = [
    Menu::File,
    Menu::Edit,
    Menu::Search,
    Menu::Run,
    Menu::Compile,
    Menu::Debug,
    Menu::Options,
    Menu::Window,
    Menu::Help,
];

impl Menu {
    fn index(self) -> usize {
        MENUS.iter().position(|&m| m == self).unwrap_or(0)
    }

    /// The Alt+letter that opens this menu (its first letter).
    fn from_hotkey(c: char) -> Option<Menu> {
        MENUS.iter().copied().find(|m| MENU_TITLES[m.index()].starts_with(c.to_ascii_uppercase()))
    }
}

/// Keys that move around menus, dialogs and the help screen.
#[derive(Debug, Clone, Copy, PartialEq)]
enum NavKey {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Tab,
    Char(char),
}

/// One entry of a drop-down menu.
struct MenuItem {
    label: String,
    /// The letter (shown in red) that runs the item while the menu is open.
    hotkey: char,
    keys: &'static str,
    /// None when the item is not available yet.
    message: Option<Message>,
}

fn item(label: impl Into<String>, hotkey: char, keys: &'static str, message: Message) -> MenuItem {
    MenuItem { label: label.into(), hotkey, keys, message: Some(message) }
}

/// Settings shown as [x] / [ ] in the Options menu.
#[derive(Debug, Clone, Copy, Default)]
struct Toggles {
    vim: bool,
    check_before: bool,
}

fn menu_items(menu: Menu, toggles: Toggles, windows: &[String], active: usize) -> Vec<MenuItem> {
    match menu {
        Menu::File => vec![
            item("New", 'n', "", Message::New),
            item("Open...", 'o', "F3", Message::Open),
            item("Save", 's', "F2", Message::Save),
            item("Save as...", 'a', "", Message::SaveAs),
            item("Exit", 'x', "Alt+X", Message::Exit),
        ],
        Menu::Edit => vec![
            item("Undo", 'u', "Ctrl+Z", Message::Undo),
            item("Redo", 'r', "Ctrl+Y", Message::Redo),
        ],
        Menu::Search => vec![
            item("Find...", 'f', "Ctrl+F", Message::OpenSearch(SearchMode::Find)),
            item("Replace...", 'r', "Ctrl+H", Message::OpenSearch(SearchMode::Replace)),
            item("Search again", 's', "Ctrl+L", Message::FindNext),
            item("Go to line number...", 'g', "Ctrl+G", Message::OpenSearch(SearchMode::GoTo)),
        ],
        Menu::Run => vec![
            item("Run", 'r', "F5", Message::Run),
            item("Run in external terminal", 'e', "", Message::RunExternal),
            item("Program reset", 'p', "Ctrl+F2", Message::OutputStop),
            item("User screen", 'u', "Alt+F5", Message::OutputToggle),
        ],
        Menu::Compile => vec![
            item("Compile", 'c', "F9", Message::Compile),
            item("Syntax check", 's', "Alt+F9", Message::SyntaxCheck),
            item("Next message", 'n', "Alt+F8", Message::NextMessage),
            item("Previous message", 'p', "Alt+F7", Message::PrevMessage),
        ],
        Menu::Debug => vec![
            item("Toggle breakpoint", 'b', "Ctrl+F8", Message::ToggleBreakpoint),
            item("Debug run", 'd', "Shift+F5", Message::DebugRun),
            item("Trace into", 't', "F7", Message::DebugStep(Step::Into)),
            item("Step over", 's', "F8", Message::DebugStep(Step::Over)),
            item("Step out", 'o', "Shift+F8", Message::DebugStep(Step::Out)),
            item("Go to cursor", 'g', "F4", Message::GoToCursor),
            item("Add watch...", 'w', "Ctrl+F7", Message::OpenSearch(SearchMode::Watch)),
            item("Clear watches", 'c', "", Message::ClearWatches),
            item("Clear breakpoints", 'l', "", Message::ClearBreakpoints),
            item("Program reset", 'r', "Ctrl+F2", Message::OutputStop),
        ],
        Menu::Options => vec![
            item("Dependencies...", 'd', "", Message::ShowDeps),
            item(if toggles.vim { "Vim mode           [x]" } else { "Vim mode           [ ]" }, 'v', "", Message::ToggleVim),
            item(
                if toggles.check_before { "Check before run   [x]" } else { "Check before run   [ ]" },
                'c',
                "",
                Message::ToggleCheckBefore,
            ),
        ],
        Menu::Window => {
            const ALT_KEYS: [&str; 9] = ["Alt+1", "Alt+2", "Alt+3", "Alt+4", "Alt+5", "Alt+6", "Alt+7", "Alt+8", "Alt+9"];
            let mut items = vec![
                item("Next", 'n', "F6", Message::NextWindow),
                item("Previous", 'p', "Shift+F6", Message::PrevWindow),
                item("Close", 'c', "Alt+F3", Message::CloseWindow),
            ];
            for (index, name) in windows.iter().enumerate() {
                let mark = if index == active { ">" } else { " " };
                let hotkey = char::from_digit((index as u32 + 1) % 10, 10).unwrap_or(' ');
                let keys = ALT_KEYS.get(index).copied().unwrap_or("");
                items.push(item(format!("{}{mark}{name}", index + 1), hotkey, keys, Message::SelectWindow(index)));
            }
            items
        }
        Menu::Help => vec![
            item("Keyboard shortcuts", 'k', "F1", Message::ToggleHelp),
            item("Install guide (GitHub)", 'i', "", Message::OpenUrl(platform::INSTALL_GUIDE_URL)),
            item("Project page (GitHub)", 'p', "", Message::OpenUrl(platform::GITHUB_URL)),
        ],
    }
}

/// Maps a key press to a navigation key.
fn nav_key(key: &Key, modifiers: Modifiers, text: Option<&str>) -> Option<NavKey> {
    use key::Named;
    match key.as_ref() {
        Key::Named(Named::ArrowUp) => Some(NavKey::Up),
        Key::Named(Named::ArrowDown) => Some(NavKey::Down),
        Key::Named(Named::ArrowLeft) => Some(NavKey::Left),
        Key::Named(Named::ArrowRight) => Some(NavKey::Right),
        Key::Named(Named::PageUp) => Some(NavKey::PageUp),
        Key::Named(Named::PageDown) => Some(NavKey::PageDown),
        Key::Named(Named::Home) => Some(NavKey::Home),
        Key::Named(Named::End) => Some(NavKey::End),
        Key::Named(Named::Enter) => Some(NavKey::Enter),
        Key::Named(Named::Tab) => Some(NavKey::Tab),
        _ if modifiers.control() || modifiers.alt() => None,
        _ => text?.chars().find(|c| !c.is_control()).map(|c| NavKey::Char(c.to_ascii_lowercase())),
    }
}

/// What to do once the file is saved (Save As may be pending).
#[derive(Debug, Clone, Copy, PartialEq)]
enum AfterSave {
    Nothing,
    /// Alt+F9: look for errors only.
    Check,
    Compile,
    Run,
    RunExternal,
    Debug(DebugStart),
}

/// How a debug session starts.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DebugStart {
    /// F7 / F8: stop at the first line of the program.
    Entry,
    /// Shift+F5: run until a breakpoint.
    Breakpoints,
    /// F4: run to this line (0-based) of the current file.
    ToLine(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Into,
    Over,
    Out,
}

/// A running debug session.
#[derive(Default)]
struct DebugSession {
    handle: Option<dap::Handle>,
    /// Where execution is stopped (file as shown in the editor, 0-based line).
    stopped_at: Option<(PathBuf, usize)>,
    frames: Vec<dap::Frame>,
    locals: Vec<dap::Variable>,
}

/// Commands that throw away unsaved edits and therefore need a second press.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Discard {
    Close,
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
    /// The row chosen with Up/Down.
    selected: usize,
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
    /// Run the syntax check first for compile and run; stop on errors.
    check_before: bool,
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
    /// The highlighted item of the open menu.
    menu_item: usize,
    /// The F1 help screen is open.
    help: bool,
    /// The Messages panel row chosen with Alt+F8 / Alt+F7.
    message_index: Option<usize>,
    /// Breakpoints: file -> 0-based lines.
    breakpoints: BTreeMap<PathBuf, BTreeSet<usize>>,
    /// Watch expressions and their last values.
    watches: Vec<(String, String)>,
    debug: Option<DebugSession>,
    debug_run_id: u64,
    /// The other open windows, in window order without the active one.
    background: Vec<Buffer>,
    /// The active window's position in the window order.
    active: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SearchMode {
    Find,
    Replace,
    GoTo,
    /// The "Add watch" input (Ctrl+F7).
    Watch,
}

struct SearchBar {
    mode: SearchMode,
    query: String,
    replacement: String,
    case: bool,
}

/// A file open in a window that is not the active one. The active window's
/// state lives directly in `Ide` (content, path, modified, undo, ...).
#[derive(Default)]
struct Buffer {
    content: text_editor::Content,
    path: Option<PathBuf>,
    modified: bool,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    live: Vec<lsp::Diagnostic>,
    marks: Arc<Vec<Mark>>,
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
    OpenMenu(Menu),
    MenuPick(usize),
    Nav(NavKey),
    ToggleHelp,
    NextMessage,
    PrevMessage,
    ToggleMatchCase,
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
    /// The bool says that only the syntax check ran.
    BuildFinished(BuildResult, AfterSave, bool),
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
    ToggleCheckBefore,
    SyntaxCheck,
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
    NextWindow,
    PrevWindow,
    CloseWindow,
    SelectWindow(usize),
    ToggleBreakpoint,
    ClearBreakpoints,
    DebugStep(Step),
    DebugRun,
    GoToCursor,
    AddWatch,
    ClearWatches,
    Debug(u64, dap::Event),
}

impl Ide {
    /// Starts empty, or with the file given on the command line, and checks
    /// the external tools in the background.
    fn new() -> (Self, Task<Message>) {
        let mut ide = Self::default();
        let os = Os::detect();
        ide.os = Some(os);
        ide.check_before = platform::read_setting("check_before").as_deref() == Some("true");
        if platform::read_setting("vim").as_deref() == Some("true") {
            ide.vim = Some(vim::Vim::default());
        }
        ide.check_at_startup =
            platform::read_setting("check_dependencies").as_deref() != Some("false");
        // Each file on the command line opens in its own window.
        let files: Vec<PathBuf> = std::env::args_os()
            .skip(1)
            .filter(|a| !a.to_string_lossy().starts_with("--"))
            .map(PathBuf::from)
            .collect();
        for path in files {
            if path.exists() {
                ide.open_path(&path);
            } else {
                if !ide.is_blank_window() {
                    ide.open_window(Buffer::default());
                }
                ide.status = format!("New file {}", path.display());
                ide.path = Some(path);
            }
        }
        if ide.window_count() > 1 {
            let _ = ide.select_window(0);
        }
        let check = if ide.check_at_startup {
            Task::perform(platform::blocking(move || deps::check(os)), |list| Message::DepsChecked(list, true))
        } else {
            Task::none()
        };
        // Like Turbo Pascal, start with the cursor in the editor, ready to type.
        (ide, Task::batch([check, iced::widget::operation::focus(EDITOR)]))
    }

    fn toggles(&self) -> Toggles {
        Toggles { vim: self.vim.is_some(), check_before: self.check_before }
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
                text,
                ..
            } => shortcut(&key, physical_key, modifiers)
                .or_else(|| nav_key(&key, modifiers, text.as_deref()).map(Message::Nav)),
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
        // A command closes an open menu; navigation and background events do not.
        let keeps_menu = matches!(
            message,
            Message::Edit(_)
                | Message::Lsp(_)
                | Message::Output(..)
                | Message::BuildFinished(..)
                | Message::DepsChecked(..)
                | Message::DepInstalled(..)
                | Message::Debug(..)
                | Message::ToggleMenu(_)
                | Message::OpenMenu(_)
                | Message::MenuPick(_)
                | Message::Nav(_)
                | Message::Escape
        );
        if !keeps_menu {
            self.menu = None;
        }
        // Background LSP events must not cancel a pending "press again to discard".
        let confirm = if matches!(message, Message::Lsp(_)) { None } else { self.confirm.take() };

        match message {
            Message::Edit(action) => self.edit(action),
            Message::ToggleMenu(menu) => {
                if self.menu == Some(menu) {
                    self.menu = None;
                    Task::none()
                } else {
                    self.open_menu(menu)
                }
            }
            Message::OpenMenu(menu) => self.open_menu(menu),
            Message::MenuPick(index) => self.pick_menu_item(index),
            Message::Nav(key) => self.navigate(key),
            Message::ToggleHelp => {
                self.help = !self.help;
                Task::none()
            }
            Message::NextMessage => {
                self.step_message(true);
                Task::none()
            }
            Message::PrevMessage => {
                self.step_message(false);
                Task::none()
            }
            Message::ToggleMatchCase => {
                if let Some(bar) = &mut self.search {
                    bar.case = !bar.case;
                }
                self.refresh_search_marks();
                Task::none()
            }
            Message::CloseMenu => Task::none(),
            Message::New => {
                if !self.is_blank_window() {
                    self.open_window(Buffer::default());
                }
                self.status = "New file".into();
                iced::widget::operation::focus(EDITOR)
            }
            Message::Open => {
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
                self.open_path(&path);
                iced::widget::operation::focus(EDITOR)
            }
            Message::NextWindow => self.select_window((self.active + 1) % self.window_count()),
            Message::PrevWindow => {
                let count = self.window_count();
                self.select_window((self.active + count - 1) % count)
            }
            Message::SelectWindow(index) => self.select_window(index),
            Message::CloseWindow => {
                if self.needs_confirm(confirm, Discard::Close) {
                    return Task::none();
                }
                self.close_window();
                iced::widget::operation::focus(EDITOR)
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
            Message::SyntaxCheck => self.save(AfterSave::Check),
            Message::Run if self.debug.is_some() => {
                self.debug_command(dap::Command::Continue);
                Task::none()
            }
            Message::Run => self.save(AfterSave::Run),
            Message::ToggleBreakpoint => {
                self.toggle_breakpoint();
                Task::none()
            }
            Message::ClearBreakpoints => {
                let files: Vec<PathBuf> = std::mem::take(&mut self.breakpoints).into_keys().collect();
                for path in files {
                    self.debug_command(dap::Command::SetBreakpoints { path, lines: Vec::new() });
                }
                self.status = "All breakpoints cleared".into();
                Task::none()
            }
            Message::DebugStep(step) => {
                if self.debug.is_some() {
                    self.debug_command(match step {
                        Step::Into => dap::Command::StepIn,
                        Step::Over => dap::Command::Next,
                        Step::Out => dap::Command::StepOut,
                    });
                    Task::none()
                } else {
                    self.save(AfterSave::Debug(DebugStart::Entry))
                }
            }
            Message::DebugRun => {
                if self.debug.is_some() {
                    self.debug_command(dap::Command::Continue);
                    Task::none()
                } else {
                    self.save(AfterSave::Debug(DebugStart::Breakpoints))
                }
            }
            Message::GoToCursor => {
                let line = self.content.cursor().position.line;
                match (&self.debug, self.path.clone()) {
                    (Some(_), Some(path)) => {
                        self.debug_command(dap::Command::RunTo { path, line: line + 1 });
                        Task::none()
                    }
                    _ => self.save(AfterSave::Debug(DebugStart::ToLine(line))),
                }
            }
            Message::AddWatch => {
                let expression = self.search.as_ref().map(|b| b.query.trim().to_string()).unwrap_or_default();
                if !expression.is_empty() && !self.watches.iter().any(|(e, _)| *e == expression) {
                    self.watches.push((expression.clone(), String::new()));
                    self.debug_command(dap::Command::Evaluate { expression });
                }
                self.close_search()
            }
            Message::ClearWatches => {
                self.watches.clear();
                Task::none()
            }
            Message::Debug(run_id, event) => {
                if run_id == self.debug_run_id {
                    return self.debug_event(event);
                }
                Task::none()
            }
            Message::RunExternal => self.save(AfterSave::RunExternal),
            Message::BuildFinished(result, after, checked_only) => {
                self.building = false;
                self.finish_build(result, after, checked_only)
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
            Message::ToggleCheckBefore => {
                self.check_before = !self.check_before;
                let on = self.check_before;
                self.status = format!("Check before compile/run {}", if on { "on" } else { "off" });
                if let Err(e) = platform::write_setting("check_before", if on { "true" } else { "false" }) {
                    self.status = format!("Cannot save settings: {e}");
                }
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
                if self.menu.is_some() {
                    self.menu = None;
                } else if self.help {
                    self.help = false;
                } else if self.deps.is_some() {
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
            Message::OutputStop if self.debug.is_some() => {
                if let Some(handle) = self.debug.as_ref().and_then(|d| d.handle.clone()) {
                    handle.send(dap::Command::Terminate);
                    handle.kill();
                }
                self.status = "Program reset".into();
                Task::none()
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
                let selected = self.deps.as_ref().map_or(0, |d| d.selected).min(list.len().saturating_sub(1));
                self.deps = Some(DepsDialog { list, selected, checking: false, installing: None, message });
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
                    selected: 0,
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

    // ----- Windows -----

    fn window_count(&self) -> usize {
        self.background.len() + 1
    }

    fn modified_windows(&self) -> usize {
        usize::from(self.modified) + self.background.iter().filter(|b| b.modified).count()
    }

    /// Window names in order, with `*` for unsaved changes.
    fn window_names(&self) -> Vec<String> {
        let name = |path: &Option<PathBuf>, modified: bool| {
            let file = path
                .as_deref()
                .and_then(Path::file_name)
                .map_or("NONAME.RS".to_string(), |n| n.to_string_lossy().into_owned());
            if modified { format!("{file}*") } else { file }
        };
        let mut names: Vec<String> = self.background.iter().map(|b| name(&b.path, b.modified)).collect();
        names.insert(self.active, name(&self.path, self.modified));
        names
    }

    /// An untitled, unchanged, empty window that a file can simply replace.
    fn is_blank_window(&self) -> bool {
        self.path.is_none() && !self.modified && self.content.text().trim().is_empty()
    }

    /// Moves the active window's state out of the editor fields.
    fn take_active(&mut self) -> Buffer {
        self.undo_group = false;
        Buffer {
            content: std::mem::take(&mut self.content),
            path: self.path.take(),
            modified: std::mem::take(&mut self.modified),
            undo: std::mem::take(&mut self.undo),
            redo: std::mem::take(&mut self.redo),
            live: std::mem::take(&mut self.live),
            marks: std::mem::take(&mut self.marks),
        }
    }

    fn put_active(&mut self, buffer: Buffer) {
        self.content = buffer.content;
        self.path = buffer.path;
        self.modified = buffer.modified;
        self.undo = buffer.undo;
        self.redo = buffer.redo;
        self.live = buffer.live;
        self.marks = buffer.marks;
        self.completion = None;
        self.message_index = None;
        self.refresh_search_marks();
        self.lsp_sync_doc();
    }

    /// Opens `buffer` in a new window right after the active one.
    fn open_window(&mut self, buffer: Buffer) {
        let current = self.take_active();
        self.background.insert(self.active, current);
        self.active += 1;
        self.put_active(buffer);
    }

    fn select_window(&mut self, index: usize) -> Task<Message> {
        if index != self.active && index < self.window_count() {
            let current = self.take_active();
            self.background.insert(self.active, current);
            let target = self.background.remove(index);
            self.active = index;
            self.put_active(target);
            let name = self.window_names()[index].clone();
            self.status = format!("Window {}: {name}", index + 1);
        }
        iced::widget::operation::focus(EDITOR)
    }

    fn close_window(&mut self) {
        if self.background.is_empty() {
            // The last window becomes an empty one.
            let _ = self.take_active();
            self.put_active(Buffer::default());
            self.status = "New file".into();
            return;
        }
        let _closed = self.take_active();
        self.active = self.active.min(self.background.len() - 1);
        let next = self.background.remove(self.active);
        self.put_active(next);
        self.status = format!("Window closed; {} open", self.window_count());
    }

    /// Shows a file: switches to its window if it is open, otherwise opens
    /// it (in the current window when that one is blank).
    fn open_path(&mut self, path: &Path) -> bool {
        if self.is_current_file(path) {
            return true;
        }
        if let Some(index) = self.window_index_of(path) {
            let _ = self.select_window(index);
            return true;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            self.status = format!("Cannot open {}", path.display());
            return false;
        };
        if !self.is_blank_window() {
            self.open_window(Buffer::default());
        }
        self.content = text_editor::Content::with_text(source.trim_start_matches('\u{feff}'));
        self.path = Some(path.to_path_buf());
        self.modified = false;
        self.undo.clear();
        self.redo.clear();
        self.set_live(Vec::new());
        self.lsp_sync_doc();
        self.status = format!("Opened {}", path.display());
        true
    }

    fn window_index_of(&self, path: &Path) -> Option<usize> {
        let position = self
            .background
            .iter()
            .position(|b| b.path.as_deref().is_some_and(|p| lsp::same_file(p, path)))?;
        // Background order skips the active window.
        Some(if position >= self.active { position + 1 } else { position })
    }

    // ----- Debugger -----

    fn debug_command(&self, command: dap::Command) {
        if let Some(handle) = self.debug.as_ref().and_then(|d| d.handle.as_ref()) {
            handle.send(command);
        }
    }

    fn toggle_breakpoint(&mut self) {
        let Some(path) = self.path.clone() else {
            self.status = "Save the file first to set breakpoints".into();
            return;
        };
        let line = self.content.cursor().position.line;
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

    fn start_debug(&mut self, target: RunTarget, start: DebugStart) -> Task<Message> {
        let Some(path) = self.path.clone() else { return Task::none() };
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let breakpoints: BTreeMap<PathBuf, Vec<usize>> = self
            .breakpoints
            .iter()
            .map(|(file, lines)| (file.clone(), lines.iter().map(|l| l + 1).collect()))
            .collect();
        let temporary = match start {
            DebugStart::ToLine(line) => Some((path.clone(), line + 1)),
            _ => None,
        };

        let launch = match Lang::from_path(Some(&path)) {
            Lang::Rust => {
                let Some((gdb, _)) = dap::find_gdb() else {
                    self.status = "Debugging Rust needs gdb 14 or newer (see Options > Dependencies)".into();
                    return Task::none();
                };
                dap::Launch {
                    adapter: dap::Adapter::Gdb,
                    command: vec![gdb.to_string_lossy().into_owned(), "-q".into(), "-i".into(), "dap".into()],
                    arguments: serde_json::json!({ "program": target.program, "cwd": target.cwd }),
                    entry_function: (start == DebugStart::Entry).then_some(target.entry.clone()).flatten(),
                    breakpoints,
                    temporary,
                }
            }
            _ => {
                let Some(python) = dap::debugpy_python(&root) else {
                    self.status = "Debugging Python needs debugpy (see Options > Dependencies)".into();
                    return Task::none();
                };
                let script = target.args.first().cloned().unwrap_or_else(|| path.to_string_lossy().into_owned());
                dap::Launch {
                    adapter: dap::Adapter::Debugpy,
                    command: vec![python.to_string_lossy().into_owned(), "-m".into(), "debugpy.adapter".into()],
                    arguments: serde_json::json!({
                        "program": script,
                        "cwd": target.cwd,
                        "console": "internalConsole",
                        "justMyCode": true,
                        "python": python,
                        "stopOnEntry": start == DebugStart::Entry,
                    }),
                    entry_function: None,
                    breakpoints,
                    temporary,
                }
            }
        };

        self.output.screen.clear();
        self.output.title = format!("{} (debugging)", path.file_name().unwrap_or_default().to_string_lossy());
        self.output.running = true;
        self.output.handle = None;
        self.debug = Some(DebugSession::default());
        self.debug_run_id += 1;
        self.status = "Debugging... (F7/F8 step, F5 continue, Ctrl+F2 stop)".into();
        let run_id = self.debug_run_id;
        Task::run(dap::run(launch), move |event| Message::Debug(run_id, event))
    }

    fn debug_event(&mut self, event: dap::Event) -> Task<Message> {
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
                    return Task::none();
                };
                if let Some(file) = &top.file
                    && !self.is_current_file(file)
                    && !self.open_path(file)
                {
                    return Task::none();
                }
                let line = top.line.saturating_sub(1);
                self.place_clamped(line, 0);
                let text = self.content.line(line).map(|l| l.text.into_owned()).unwrap_or_default();
                self.place_clamped(line, text.len() - text.trim_start().len());
                if let (Some(debug), Some(path)) = (&mut self.debug, self.path.clone()) {
                    debug.stopped_at = Some((path, line));
                }
                self.status = format!("Stopped at line {} in {}", top.line, top.name);
                for (expression, _) in self.watches.clone() {
                    self.debug_command(dap::Command::Evaluate { expression });
                }
                return iced::widget::operation::focus(EDITOR);
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
        Task::none()
    }

    /// Diagnostics, breakpoints, the execution line and search highlights.
    fn editor_marks(&self) -> Arc<Vec<Mark>> {
        let breakpoints = self.path.as_ref().and_then(|p| self.breakpoints.get(p));
        let exec = self
            .debug
            .as_ref()
            .and_then(|d| d.stopped_at.as_ref())
            .filter(|(file, _)| self.path.as_ref() == Some(file))
            .map(|(_, line)| *line);
        if breakpoints.is_none() && exec.is_none() && self.search_marks.is_empty() {
            return self.marks.clone();
        }
        let whole = |line: usize, kind| Mark { line, start: 0, end: usize::MAX / 2, kind };
        let mut marks: Vec<Mark> = breakpoints
            .into_iter()
            .flatten()
            .map(|&line| whole(line, MarkKind::Breakpoint))
            .collect();
        marks.extend(self.marks.iter().cloned());
        marks.extend(exec.map(|line| whole(line, MarkKind::ExecLine)));
        marks.extend(self.search_marks.iter().cloned());
        Arc::new(marks)
    }

    /// One tab per open window; F6 / Alt+number switch between them.
    fn window_tabs(&self) -> Element<'_, Message> {
        let tabs = self.window_names().into_iter().enumerate().map(|(index, name)| {
            let active = index == self.active;
            button(text(format!(" {} {name} ", index + 1)).color(if active { Color::BLACK } else { TURBO_DARK_GRAY }))
                .padding([1, 4])
                .on_press(Message::SelectWindow(index))
                .style(move |_, status| button::Style {
                    background: Some(Background::Color(
                        if active || matches!(status, button::Status::Hovered) { TURBO_CYAN } else { TURBO_GRAY },
                    )),
                    ..button::Style::default()
                })
                .into()
        });
        container(row(tabs).spacing(2))
            .width(Length::Fill)
            .padding([1, 4])
            .style(|_| container::Style::default().background(TURBO_GRAY))
            .into()
    }

    /// Watches, locals and the call stack, shown while debugging.
    fn debug_panel(&self) -> Element<'_, Message> {
        let header = |title: &str| text(format!(" {title}")).color(Color::WHITE);
        let line = |left: String, right: String| {
            row![text(left).color(Color::BLACK).width(Length::FillPortion(2)), text(right).color(TURBO_BLUE).width(Length::FillPortion(3))]
                .spacing(6)
        };
        let watches = column(
            self.watches
                .iter()
                .map(|(expr, value)| line(expr.clone(), value.clone()).into())
                .collect::<Vec<Element<'_, Message>>>(),
        );
        let (locals, frames) = match &self.debug {
            Some(debug) => (
                column(
                    debug
                        .locals
                        .iter()
                        .map(|v| line(v.name.clone(), truncate(&v.value, 60)).into())
                        .collect::<Vec<Element<'_, Message>>>(),
                ),
                column(
                    debug
                        .frames
                        .iter()
                        .map(|f| {
                            let file = f.file.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
                            let place = file.map(|n| format!("{n}:{}", f.line)).unwrap_or_default();
                            text(format!("{}  {place}", truncate(&f.name, 40))).color(Color::BLACK).into()
                        })
                        .collect::<Vec<Element<'_, Message>>>(),
                ),
            ),
            None => (column![text(" (not running)").color(TURBO_DARK_GRAY)], column![]),
        };
        let section = |title: &str, body: iced::widget::Column<'static, Message>| {
            column![header(title), scrollable(body.padding([0, 6])).height(Length::Fill)].width(Length::Fill)
        };
        container(
            row![
                section("Watches  (Ctrl+F7 add)", watches),
                section("Locals", locals),
                section("Call stack", frames),
            ]
            .spacing(4),
        )
        .height(MESSAGES_HEIGHT)
        .width(Length::Fill)
        .style(|_| container::Style::default().background(TURBO_CYAN))
        .into()
    }

    // ----- Keyboard navigation -----

    fn open_menu(&mut self, menu: Menu) -> Task<Message> {
        self.menu = Some(menu);
        let items = menu_items(menu, self.toggles(), &self.window_names(), self.active);
        self.menu_item = items.iter().position(|i| i.message.is_some()).unwrap_or(0);
        // Arrow keys must reach the menu, not a text field.
        iced::widget::operation::focus(EDITOR)
    }

    fn pick_menu_item(&mut self, index: usize) -> Task<Message> {
        let Some(menu) = self.menu else { return Task::none() };
        let message = menu_items(menu, self.toggles(), &self.window_names(), self.active).into_iter().nth(index).and_then(|i| i.message);
        self.menu = None;
        match message {
            Some(message) => self.update(message),
            None => Task::none(),
        }
    }

    /// Arrow keys, Enter and letters for whatever modal thing is open.
    fn navigate(&mut self, key: NavKey) -> Task<Message> {
        if let Some(menu) = self.menu {
            return self.menu_key(menu, key);
        }
        if self.help {
            use iced::widget::scrollable::{AbsoluteOffset, RelativeOffset};
            let scroll = |y| iced::widget::operation::scroll_by(HELP_SCROLL, AbsoluteOffset { x: 0.0, y });
            return match key {
                NavKey::Up => scroll(-24.0),
                NavKey::Down => scroll(24.0),
                NavKey::PageUp => scroll(-360.0),
                NavKey::PageDown => scroll(360.0),
                NavKey::Home => iced::widget::operation::snap_to(HELP_SCROLL, RelativeOffset::START),
                NavKey::End => iced::widget::operation::snap_to(HELP_SCROLL, RelativeOffset::END),
                NavKey::Enter => {
                    self.help = false;
                    Task::none()
                }
                _ => Task::none(),
            };
        }
        if self.deps.is_some() {
            return self.deps_key(key);
        }
        if self.search.is_some() && key == NavKey::Tab {
            return iced::widget::operation::focus_next();
        }
        Task::none()
    }

    fn menu_key(&mut self, menu: Menu, key: NavKey) -> Task<Message> {
        let items = menu_items(menu, self.toggles(), &self.window_names(), self.active);
        let enabled: Vec<usize> = (0..items.len()).filter(|&i| items[i].message.is_some()).collect();
        let step = |by: isize| {
            let at = MENUS.len() as isize + menu.index() as isize + by;
            MENUS[(at as usize) % MENUS.len()]
        };
        let move_item = |current: usize, down: bool| -> usize {
            if enabled.is_empty() {
                return current;
            }
            let pos = enabled.iter().position(|&i| i == current).unwrap_or(0);
            let next = if down { (pos + 1) % enabled.len() } else { (pos + enabled.len() - 1) % enabled.len() };
            enabled[next]
        };
        match key {
            NavKey::Left => self.open_menu(step(-1)),
            NavKey::Right | NavKey::Tab => self.open_menu(step(1)),
            NavKey::Up => {
                self.menu_item = move_item(self.menu_item, false);
                Task::none()
            }
            NavKey::Down => {
                self.menu_item = move_item(self.menu_item, true);
                Task::none()
            }
            NavKey::Home | NavKey::PageUp => {
                self.menu_item = enabled.first().copied().unwrap_or(0);
                Task::none()
            }
            NavKey::End | NavKey::PageDown => {
                self.menu_item = enabled.last().copied().unwrap_or(0);
                Task::none()
            }
            NavKey::Enter => self.pick_menu_item(self.menu_item),
            NavKey::Char(c) => match items.iter().position(|i| i.hotkey == c && i.message.is_some()) {
                Some(index) => self.pick_menu_item(index),
                None => Task::none(),
            },
        }
    }

    fn deps_key(&mut self, key: NavKey) -> Task<Message> {
        let Some(dialog) = &mut self.deps else { return Task::none() };
        let count = dialog.list.len();
        let busy = dialog.checking || dialog.installing.is_some();
        match key {
            NavKey::Up if count > 0 => dialog.selected = (dialog.selected + count - 1) % count,
            NavKey::Down if count > 0 => dialog.selected = (dialog.selected + 1) % count,
            NavKey::Home => dialog.selected = 0,
            NavKey::End => dialog.selected = count.saturating_sub(1),
            _ => {}
        }
        let selected = dialog.selected;
        let dep = dialog.list.get(selected).filter(|d| d.missing());
        let can_install = dep.is_some_and(|d| d.auto.is_some()) && !busy;
        let install_text = dep.map(|d| d.install.clone());
        match key {
            NavKey::Char('i') if can_install => self.update(Message::DepInstall(selected)),
            NavKey::Enter if can_install => self.update(Message::DepInstall(selected)),
            NavKey::Char('c') | NavKey::Enter => match install_text {
                Some(text) => self.update(Message::CopyText(text)),
                None => Task::none(),
            },
            NavKey::Char('g') => self.update(Message::OpenUrl(platform::INSTALL_GUIDE_URL)),
            NavKey::Char('r') if !busy => self.update(Message::ShowDeps),
            NavKey::Char('s') => self.update(Message::ToggleStartupCheck(!self.check_at_startup)),
            _ => Task::none(),
        }
    }

    /// Alt+F8 / Alt+F7: jump to the next or previous Messages row.
    fn step_message(&mut self, forward: bool) {
        let items = self.panel_items();
        if items.is_empty() {
            self.status = "No messages".into();
            return;
        }
        let count = items.len();
        let index = match self.message_index {
            Some(i) if forward => (i + 1) % count,
            Some(i) => (i.min(count - 1) + count - 1) % count,
            None if forward => 0,
            None => count - 1,
        };
        self.message_index = Some(index);
        let item = items[index].clone();
        self.status = format!("{} {}/{}: {}", item.level.to_uppercase(), index + 1, count, item.message);
        self.jump_to(&item);
    }

    // ----- Search -----

    fn open_search(&mut self, mode: SearchMode) -> Task<Message> {
        // Start from the selected text (one line only), else the last search.
        let selected = self.content.selection().filter(|s| !s.contains('\n') && !s.is_empty());
        let query = match mode {
            SearchMode::GoTo => String::new(),
            SearchMode::Watch => selected.unwrap_or_default(),
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
            && matches!(bar.mode, SearchMode::Find | SearchMode::Replace)
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
            && matches!(bar.mode, SearchMode::Find | SearchMode::Replace)
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
        if matches!(bar.mode, SearchMode::GoTo | SearchMode::Watch) || bar.query.is_empty() {
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
            SearchMode::Watch => row![
                label(" Add watch:"),
                input(&bar.query, Message::SearchQuery, Message::AddWatch, 300.0).id(SEARCH_INPUT),
                small_button("Add", Some(Message::AddWatch)),
                text("an expression, evaluated at each stop").color(TURBO_DARK_GRAY),
                Space::new().width(Length::Fill),
                close,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into(),
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
                        .label("Match case (Alt+M)")
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
        let unsaved = match command {
            Discard::Close => self.modified,
            Discard::Exit => self.modified_windows() > 0,
        };
        if !unsaved || previous == Some(command) {
            return false;
        }
        let key = match command {
            Discard::Close => "Alt+F3",
            Discard::Exit => "Alt+X",
        };
        let what = if command == Discard::Exit && self.modified_windows() > 1 {
            format!("{} windows have unsaved changes", self.modified_windows())
        } else {
            "Unsaved changes".to_string()
        };
        self.status = format!("{what}! Press F2 to save, or {key} again to discard.");
        self.confirm = Some(command);
        true
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
        let verb = if after == AfterSave::Check { "Checking" } else { "Compiling" };
        self.status = format!("{verb} {} ...", path.display());
        let check_first = self.check_before;
        Task::perform(platform::blocking(move || run_build(&path, after, check_first)), move |(result, checked_only)| {
            Message::BuildFinished(result, after, checked_only)
        })
    }

    fn finish_build(&mut self, result: BuildResult, after: AfterSave, checked_only: bool) -> Task<Message> {
        self.diagnostics = result.diagnostics;
        self.message_index = None;
        let errors = self.diagnostics.iter().filter(|d| d.level == "error").count();
        let warnings = self.diagnostics.len() - errors;

        if !result.success {
            self.status = if checked_only {
                // The status bar clips long text, so the important part comes first.
                let lead = if after == AfterSave::Check { "Syntax check failed" } else { "Not built: syntax errors" };
                format!("{lead}: {errors} error(s), {warnings} warning(s)")
            } else {
                format!("Compile failed: {errors} error(s), {warnings} warning(s)")
            };
            if let Some(first) = self.diagnostics.iter().find(|d| d.level == "error").cloned() {
                self.jump_to(&first);
            }
            return Task::none();
        }

        if after == AfterSave::Check {
            self.status = if warnings == 0 {
                "No syntax errors".into()
            } else {
                format!("No syntax errors, {warnings} warning(s)")
            };
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
            AfterSave::Debug(start) => self.start_debug(target, start),
            AfterSave::Nothing | AfterSave::Check | AfterSave::Compile => Task::none(),
        }
    }

    fn jump_to(&mut self, diagnostic: &Diagnostic) {
        if diagnostic.line == 0 {
            return;
        }
        if let Some(file) = &diagnostic.file
            && !self.is_current_file(file)
            && !self.open_path(file)
        {
            return;
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
        let menu_bar = container(row(MENUS.iter().map(|&menu| {
            let title = MENU_TITLES[menu.index()];
            let open = self.menu == Some(menu);
            button(hotkey_label(title, title.chars().next().unwrap_or(' ')))
                .width(MENU_TITLE_WIDTH)
                .padding([4, 10])
                .on_press(Message::ToggleMenu(menu))
                .style(move |_, status| menu_title_style(open, status))
                .into()
        })))
        .width(Length::Fill)
        .style(|_| container::Style::default().background(TURBO_GRAY));

        let popup_open = self.completion.is_some();
        // While a menu, dialog or the help screen is open, keys drive it, not the text.
        let modal = self.menu.is_some() || self.help || self.deps.is_some();
        let vim_mode = self.vim.as_ref().map(|v| (v.mode, v.command.is_some()));
        let editor = text_editor(&self.content)
            .id(EDITOR)
            .on_action(Message::Edit)
            .height(Length::Fill)
            .highlight_with::<highlight::Highlighter>(
                highlight::Settings {
                    lang: Lang::from_path(self.path.as_deref()),
                    marks: self.editor_marks(),
                },
                highlight::to_format,
            )
            .key_binding(move |press: KeyPress| {
                if modal {
                    if let Some(message) = shortcut(&press.key, press.physical_key, press.modifiers) {
                        return Some(Binding::Custom(message));
                    }
                    return nav_key(&press.key, press.modifiers, press.text.as_deref())
                        .map(|key| Binding::Custom(Message::Nav(key)));
                }
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
                if let Key::Named(key::Named::Tab) = press.key.as_ref() {
                    let edit = if press.modifiers.shift() { Edit::Unindent } else { Edit::Indent };
                    return Some(Binding::Custom(Message::Edit(Action::Edit(edit))));
                }
                Binding::from_key_press(press)
            })
            .style(|theme, status| text_editor::Style {
                background: Background::Color(TURBO_BLUE),
                value: TURBO_YELLOW,
                ..text_editor::default(theme, status)
            });

        let editor = cursor::BlockCursor::new(editor, &self.content, TURBO_YELLOW, TURBO_BLUE);

        let panel = self.panel_items();
        let mut layout = column![menu_bar];
        if self.window_count() > 1 {
            layout = layout.push(self.window_tabs());
        }
        if let Some(bar) = &self.search {
            layout = layout.push(self.search_bar(bar));
        }
        layout = layout.push(editor);
        if self.debug.is_some() || !self.watches.is_empty() {
            layout = layout.push(self.debug_panel());
        } else if !panel.is_empty() {
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
        if self.help {
            layers = layers.push(help_screen());
        }
        if let Some(menu) = self.menu {
            layers = layers
                .push(
                    mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                        .on_press(Message::CloseMenu),
                )
                .push(dropdown(menu, self.toggles(), self.menu_item, &self.window_names(), self.active));
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
            let selected = index == dialog.selected;
            container(
                row![
                    text(mark).color(color).width(64),
                    column![
                        text(dep.name).color(Color::BLACK),
                        text(format!("{} ({need})", dep.purpose)).color(TURBO_DARK_GRAY).size(12),
                    ]
                    .width(300),
                    detail,
                ]
                .spacing(8),
            )
            .padding([2, 4])
            .style(move |_| {
                if selected {
                    container::Style::default().background(TURBO_CYAN)
                } else {
                    container::Style::default()
                }
            })
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
            column(rows).spacing(6).padding(10),
            container(text(message).color(TURBO_RED)).padding([0, 10]),
            container(
                text(" Up/Down select   I install   C copy   G guide   R re-check   S startup check   Esc close")
                    .color(TURBO_DARK_GRAY),
            )
            .padding([4, 10]),
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
        let title = if self.diagnostics.is_empty() {
            " Messages (live)   Alt+F8 next  Alt+F7 previous "
        } else {
            " Messages   Alt+F8 next  Alt+F7 previous "
        };
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
            let chosen = self.message_index == Some(index);
            button(text(format!("{} {location}{}", d.level.to_uppercase(), d.message)).color(color))
                .width(Length::Fill)
                .padding([1, 8])
                .on_press(Message::JumpTo(index))
                .style(move |_, status| button::Style {
                    background: (chosen || matches!(status, button::Status::Hovered))
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
        Key::Named(key::Named::F1) => Some(Message::ToggleHelp),
        Key::Named(key::Named::F10) => Some(Message::OpenMenu(Menu::File)),
        Key::Named(key::Named::F7) if modifiers.alt() => Some(Message::PrevMessage),
        Key::Named(key::Named::F7) if modifiers.control() => Some(Message::OpenSearch(SearchMode::Watch)),
        Key::Named(key::Named::F7) => Some(Message::DebugStep(Step::Into)),
        Key::Named(key::Named::F8) if modifiers.control() => Some(Message::ToggleBreakpoint),
        Key::Named(key::Named::F8) if modifiers.shift() => Some(Message::DebugStep(Step::Out)),
        Key::Named(key::Named::F8) if !modifiers.alt() => Some(Message::DebugStep(Step::Over)),
        Key::Named(key::Named::F4) => Some(Message::GoToCursor),
        Key::Named(key::Named::F5) if modifiers.shift() => Some(Message::DebugRun),
        Key::Named(key::Named::F8) if modifiers.alt() => Some(Message::NextMessage),
        Key::Named(key::Named::F9) if modifiers.alt() => Some(Message::SyntaxCheck),
        Key::Named(key::Named::F2) if modifiers.control() => Some(Message::OutputStop),
        Key::Named(key::Named::F2) => Some(Message::Save),
        Key::Named(key::Named::F3) if modifiers.alt() => Some(Message::CloseWindow),
        Key::Named(key::Named::F3) => Some(Message::Open),
        Key::Named(key::Named::F6) if modifiers.shift() => Some(Message::PrevWindow),
        Key::Named(key::Named::F6) => Some(Message::NextWindow),
        Key::Named(key::Named::F5) if modifiers.alt() => Some(Message::OutputToggle),
        Key::Named(key::Named::F5) => Some(Message::Run),
        Key::Named(key::Named::F9) if modifiers.control() => Some(Message::Run),
        Key::Named(key::Named::F9) => Some(Message::Compile),
        Key::Named(key::Named::Space) if modifiers.control() => Some(Message::TriggerCompletion),
        _ => match key.to_latin(physical) {
            Some('x') if modifiers.alt() => Some(Message::Exit),
            Some('m') if modifiers.alt() => Some(Message::ToggleMatchCase),
            Some('0') if modifiers.alt() => Some(Message::OpenMenu(Menu::Window)),
            Some(c @ '1'..='9') if modifiers.alt() => Some(Message::SelectWindow(c as usize - '1' as usize)),
            Some(c) if modifiers.alt() && !modifiers.control() => Menu::from_hotkey(c).map(Message::OpenMenu),
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

/// Runs what a command needs: only the syntax check (Alt+F9), or the build,
/// preceded by the check when "Check before compile/run" is on. The bool says
/// that only the check ran (asked for, or it found errors that stop the build).
fn run_build(path: &Path, after: AfterSave, check_first: bool) -> (BuildResult, bool) {
    if after == AfterSave::Check {
        return (runner::check(path), true);
    }
    if check_first {
        let checked = runner::check(path);
        if !checked.success {
            return (checked, true);
        }
    }
    (runner::build(path), false)
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

fn dropdown(menu: Menu, toggles: Toggles, selected: usize, windows: &[String], active: usize) -> Element<'static, Message> {
    let entries = menu_items(menu, toggles, windows, active).into_iter().enumerate().map(|(index, item)| {
        let enabled = item.message.is_some();
        let label: Element<'static, Message> = if enabled {
            hotkey_label(&item.label, item.hotkey)
        } else {
            text(item.label).color(TURBO_DARK_GRAY).into()
        };
        let chosen = index == selected && enabled;
        button(row![label, Space::new().width(Length::Fill), text(item.keys).color(Color::BLACK)])
            .width(290)
            .padding([3, 10])
            .on_press_maybe(enabled.then_some(Message::MenuPick(index)))
            .style(move |_, status| button::Style {
                background: Some(Background::Color(
                    if chosen || (enabled && matches!(status, button::Status::Hovered)) {
                        TURBO_GREEN
                    } else {
                        TURBO_GRAY
                    },
                )),
                ..button::Style::default()
            })
            .into()
    });

    container(container(column(entries)).style(|_| container::Style {
        border: iced::Border::default().color(Color::BLACK).width(1),
        ..container::Style::default().background(TURBO_GRAY)
    }))
    .padding(Padding::default().top(MENU_BAR_HEIGHT).left(menu.index() as f32 * MENU_TITLE_WIDTH))
    .into()
}

/// A label with its hotkey letter in red, Turbo Pascal style.
fn hotkey_label(label: &str, hotkey: char) -> Element<'static, Message> {
    let at = label.char_indices().find(|(_, c)| c.eq_ignore_ascii_case(&hotkey)).map(|(i, _)| i);
    match at {
        Some(i) => {
            let end = i + label[i..].chars().next().map_or(0, char::len_utf8);
            row![
                text(label[..i].to_string()).color(Color::BLACK),
                text(label[i..end].to_string()).color(TURBO_RED),
                text(label[end..].to_string()).color(Color::BLACK),
            ]
            .into()
        }
        None => text(label.to_string()).color(Color::BLACK).into(),
    }
}

/// F1: every keyboard shortcut, grouped.
fn help_screen() -> Element<'static, Message> {
    let mut lines: Vec<Element<'static, Message>> = Vec::new();
    let mut group = "";
    for &(g, keys, action) in SHORTCUTS {
        if g != group {
            group = g;
            lines.push(text(format!(" {g}")).color(TURBO_YELLOW).into());
        }
        lines.push(
            row![text(format!("   {keys}")).color(Color::WHITE).width(330), text(action).color(SCREEN_TEXT)].into(),
        );
    }
    let title = container(
        text(" Keyboard shortcuts    Up/Down/PgUp/PgDn scroll    Esc or F1 closes ").color(Color::BLACK),
    )
    .width(Length::Fill)
    .style(|_| container::Style::default().background(TURBO_GRAY));
    let list = scrollable(column(lines).spacing(2).padding([6, 8])).id(HELP_SCROLL).height(Length::Fill);
    let window = container(column![title, list]).style(|_| container::Style {
        border: iced::Border::default().color(TURBO_GRAY).width(2),
        ..container::Style::default().background(TURBO_CYAN)
    });
    opaque(
        container(window)
            .padding(Padding::default().top(MENU_BAR_HEIGHT + 8.0).bottom(STATUS_BAR_HEIGHT + 8.0).left(60.0).right(60.0))
            .width(Length::Fill)
            .height(Length::Fill),
    )
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

#[cfg(test)]
mod keyboard_tests {
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

    #[test]
    fn menus_open_move_and_run_items() {
        let mut ide = ide("text");
        send(&mut ide, [Message::OpenMenu(Menu::Search), Message::Nav(NavKey::Down)]);
        assert_eq!((ide.menu, ide.menu_item), (Some(Menu::Search), 1));
        send(&mut ide, [Message::Nav(NavKey::Enter)]);
        assert!(ide.menu.is_none());
        assert_eq!(ide.search.as_ref().map(|b| b.mode), Some(SearchMode::Replace));

        send(&mut ide, [Message::OpenMenu(Menu::Search), Message::Nav(NavKey::Char('g'))]);
        assert_eq!(ide.search.as_ref().map(|b| b.mode), Some(SearchMode::GoTo), "hotkey letter");
    }

    #[test]
    fn menu_left_right_wrap_and_skip_disabled() {
        let mut ide = ide("");
        send(&mut ide, [Message::OpenMenu(Menu::File), Message::Nav(NavKey::Left)]);
        assert_eq!(ide.menu, Some(Menu::Help));
        send(&mut ide, [Message::Nav(NavKey::Right)]);
        assert_eq!(ide.menu, Some(Menu::File));
        send(&mut ide, [Message::Nav(NavKey::Up)]);
        assert_eq!(ide.menu_item, 4, "Up from the first item wraps to Exit");

        send(&mut ide, [Message::OpenMenu(Menu::Debug), Message::Nav(NavKey::Enter)]);
        assert!(ide.menu.is_none(), "a disabled item does nothing but close");
    }

    #[test]
    fn alt_letters_and_function_keys_map_to_menus_and_help() {
        let alt = Modifiers::ALT;
        let key = |c: &str| Key::Character(c.into());
        let physical = key::Physical::Unidentified(key::NativeCode::Unidentified);
        assert!(matches!(shortcut(&key("s"), physical, alt), Some(Message::OpenMenu(Menu::Search))));
        assert!(matches!(shortcut(&key("h"), physical, alt), Some(Message::OpenMenu(Menu::Help))));
        assert!(matches!(shortcut(&key("x"), physical, alt), Some(Message::Exit)));
        assert!(matches!(shortcut(&key("m"), physical, alt), Some(Message::ToggleMatchCase)));
        let f = |n| Key::Named(n);
        assert!(matches!(shortcut(&f(key::Named::F1), physical, Modifiers::empty()), Some(Message::ToggleHelp)));
        assert!(matches!(shortcut(&f(key::Named::F10), physical, Modifiers::empty()), Some(Message::OpenMenu(Menu::File))));
        assert!(matches!(shortcut(&f(key::Named::F8), physical, alt), Some(Message::NextMessage)));
    }

    #[test]
    fn escape_closes_menu_then_help() {
        let mut ide = ide("");
        send(&mut ide, [Message::ToggleHelp, Message::OpenMenu(Menu::File)]);
        send(&mut ide, [Message::Escape]);
        assert!(ide.menu.is_none() && ide.help);
        send(&mut ide, [Message::Escape]);
        assert!(!ide.help);
    }

    #[test]
    fn messages_step_with_alt_f8_and_f7() {
        let mut ide = ide("a\nb\nc\n");
        let diagnostic = |line| Diagnostic {
            file: None,
            line,
            column: 1,
            level: "error".into(),
            message: format!("bad {line}"),
        };
        ide.diagnostics = vec![diagnostic(2), diagnostic(3)];
        send(&mut ide, [Message::NextMessage]);
        assert_eq!(ide.content.cursor().position.line, 1);
        assert!(ide.status.starts_with("ERROR 1/2"));
        send(&mut ide, [Message::NextMessage]);
        assert_eq!(ide.content.cursor().position.line, 2);
        send(&mut ide, [Message::NextMessage]);
        assert_eq!(ide.message_index, Some(0), "wraps around");
        send(&mut ide, [Message::PrevMessage]);
        assert_eq!(ide.message_index, Some(1));
    }

    #[test]
    fn dependency_dialog_keys() {
        let mut ide = ide("");
        let dep = |name, found: Option<&str>| deps::Dep {
            name,
            purpose: "",
            need: deps::Need::Recommended,
            found: found.map(String::from),
            install: format!("install {name}"),
            auto: None,
        };
        ide.deps = Some(DepsDialog {
            list: vec![dep("one", Some("1.0")), dep("two", None)],
            selected: 0,
            checking: false,
            installing: None,
            message: String::new(),
        });
        send(&mut ide, [Message::Nav(NavKey::Down)]);
        assert_eq!(ide.deps.as_ref().map(|d| d.selected), Some(1));
        send(&mut ide, [Message::Nav(NavKey::Char('c'))]);
        assert_eq!(ide.status, "Command copied to the clipboard");
        send(&mut ide, [Message::Nav(NavKey::Down)]);
        assert_eq!(ide.deps.as_ref().map(|d| d.selected), Some(0), "wraps around");
    }

    #[test]
    fn tab_indents_and_shift_tab_unindents() {
        let mut ide = ide("x");
        send(&mut ide, [Message::Edit(Action::Edit(Edit::Indent))]);
        let indented = ide.content.text();
        assert!(indented.ends_with('x') && indented.len() > 1, "{indented:?}");
        send(&mut ide, [Message::Edit(Action::Edit(Edit::Unindent))]);
        assert_eq!(ide.content.text(), "x");
    }
}

#[cfg(test)]
mod debug_tests {
    use super::*;

    fn ide_with_file(name: &str, text: &str) -> (Ide, PathBuf) {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("debug-ui-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        let mut ide = Ide::default();
        ide.open_path(&path);
        (ide, path)
    }

    fn send(ide: &mut Ide, messages: impl IntoIterator<Item = Message>) {
        for message in messages {
            let _ = ide.update(message);
        }
    }

    fn kinds(ide: &Ide) -> Vec<(usize, MarkKind)> {
        ide.editor_marks().iter().map(|m| (m.line, m.kind)).collect()
    }

    #[test]
    fn breakpoints_toggle_and_show() {
        let (mut ide, path) = ide_with_file("bp.rs", "fn main() {\n    let x = 1;\n}\n");
        vim::place(&mut ide.content, Position { line: 1, column: 0 });
        send(&mut ide, [Message::ToggleBreakpoint]);
        assert_eq!(ide.breakpoints.get(&path).map(|s| s.iter().copied().collect::<Vec<_>>()), Some(vec![1]));
        assert_eq!(kinds(&ide), [(1, MarkKind::Breakpoint)]);
        send(&mut ide, [Message::ToggleBreakpoint]);
        assert!(ide.breakpoints.is_empty());

        let mut unsaved = Ide::default();
        send(&mut unsaved, [Message::ToggleBreakpoint]);
        assert!(unsaved.status.starts_with("Save the file first"));
    }

    #[test]
    fn stop_events_move_the_cursor_and_fill_the_panel() {
        let (mut ide, path) = ide_with_file("stop.rs", "fn main() {\n    let x = 41;\n    let y = x + 1;\n}\n");
        ide.debug = Some(DebugSession::default());
        let frame = dap::Frame { name: "stop::main".into(), file: Some(path.clone()), line: 3 };
        let local = dap::Variable { name: "x".into(), value: "41".into() };
        send(&mut ide, [Message::Debug(0, dap::Event::Stopped { frames: vec![frame], locals: vec![local] })]);
        assert_eq!(ide.content.cursor().position, Position { line: 2, column: 4 }, "first non-blank of line 3");
        assert!(kinds(&ide).contains(&(2, MarkKind::ExecLine)));
        assert_eq!(ide.status, "Stopped at line 3 in stop::main");
        let _ = ide.debug_panel(); // builds without panicking

        send(&mut ide, [
            Message::OpenSearch(SearchMode::Watch),
            Message::SearchQuery("x + 1".into()),
            Message::AddWatch,
        ]);
        assert_eq!(ide.watches, [("x + 1".to_string(), String::new())]);
        assert!(ide.search.is_none());
        send(&mut ide, [Message::Debug(0, dap::Event::Evaluated { expression: "x + 1".into(), value: "42".into() })]);
        assert_eq!(ide.watches[0].1, "42");

        send(&mut ide, [Message::Debug(0, dap::Event::Running)]);
        assert!(!kinds(&ide).contains(&(2, MarkKind::ExecLine)), "no execution line while running");
        send(&mut ide, [Message::Debug(0, dap::Event::Exited(Some(0))), Message::Debug(0, dap::Event::Ended)]);
        assert!(ide.debug.is_none());
        assert!(ide.output.screen.contains("exit code 0"));

        send(&mut ide, [Message::Debug(7, dap::Event::Ended)]);
        assert!(ide.debug.is_none(), "events from an old session are ignored");
    }

    #[test]
    fn debug_keys() {
        let none = Modifiers::empty();
        let physical = key::Physical::Unidentified(key::NativeCode::Unidentified);
        let f = |n| Key::Named(n);
        let map = |k, m| shortcut(&f(k), physical, m);
        assert!(matches!(map(key::Named::F7, none), Some(Message::DebugStep(Step::Into))));
        assert!(matches!(map(key::Named::F8, none), Some(Message::DebugStep(Step::Over))));
        assert!(matches!(map(key::Named::F8, Modifiers::SHIFT), Some(Message::DebugStep(Step::Out))));
        assert!(matches!(map(key::Named::F8, Modifiers::CTRL), Some(Message::ToggleBreakpoint)));
        assert!(matches!(map(key::Named::F8, Modifiers::ALT), Some(Message::NextMessage)));
        assert!(matches!(map(key::Named::F7, Modifiers::CTRL), Some(Message::OpenSearch(SearchMode::Watch))));
        assert!(matches!(map(key::Named::F4, none), Some(Message::GoToCursor)));
        assert!(matches!(map(key::Named::F5, Modifiers::SHIFT), Some(Message::DebugRun)));
        assert!(matches!(map(key::Named::F5, none), Some(Message::Run)));
    }

    #[test]
    fn run_continues_while_debugging() {
        let (mut ide, _) = ide_with_file("cont.py", "print(1)\n");
        ide.debug = Some(DebugSession::default());
        send(&mut ide, [Message::Run]);
        assert!(!ide.building, "F5 continues the session instead of rebuilding");
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;

    fn files(names: &[&str]) -> Vec<PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("window-tests");
        std::fs::create_dir_all(&dir).unwrap();
        names
            .iter()
            .map(|name| {
                let path = dir.join(name);
                std::fs::write(&path, format!("// {name}\n")).unwrap();
                path
            })
            .collect()
    }

    fn send(ide: &mut Ide, messages: impl IntoIterator<Item = Message>) {
        for message in messages {
            let _ = ide.update(message);
        }
    }

    #[test]
    fn opening_files_makes_windows_and_reuses_a_blank_one() {
        let paths = files(&["a.rs", "b.rs"]);
        let mut ide = Ide::default();
        send(&mut ide, [Message::Opened(Some(paths[0].clone()))]);
        assert_eq!(ide.window_count(), 1, "the blank window is reused");
        send(&mut ide, [Message::Opened(Some(paths[1].clone()))]);
        assert_eq!(ide.window_names(), ["a.rs", "b.rs"]);
        assert_eq!(ide.active, 1);
        send(&mut ide, [Message::Opened(Some(paths[0].clone()))]);
        assert_eq!((ide.window_count(), ide.active), (2, 0), "an open file is switched to");
        assert_eq!(ide.content.text(), "// a.rs\n");
    }

    #[test]
    fn each_window_keeps_its_own_text_and_undo() {
        let paths = files(&["c.rs", "d.rs"]);
        let mut ide = Ide::default();
        send(&mut ide, [Message::Opened(Some(paths[0].clone())), Message::Opened(Some(paths[1].clone()))]);
        send(&mut ide, [Message::Edit(Action::Edit(Edit::Insert('x')))]);
        assert_eq!(ide.window_names(), ["c.rs", "d.rs*"]);
        send(&mut ide, [Message::PrevWindow]);
        assert_eq!(ide.content.text(), "// c.rs\n");
        send(&mut ide, [Message::Undo]);
        assert_eq!(ide.content.text(), "// c.rs\n", "nothing to undo in this window");
        send(&mut ide, [Message::NextWindow, Message::Undo]);
        assert_eq!(ide.content.text(), "// d.rs\n", "undo works in its own window");
    }

    #[test]
    fn closing_asks_for_unsaved_changes() {
        let paths = files(&["e.rs", "f.rs", "g.rs"]);
        let mut ide = Ide::default();
        for path in &paths {
            send(&mut ide, [Message::Opened(Some(path.clone()))]);
        }
        send(&mut ide, [Message::SelectWindow(1), Message::Edit(Action::Edit(Edit::Insert('x')))]);
        send(&mut ide, [Message::CloseWindow]);
        assert_eq!(ide.window_count(), 3, "first Alt+F3 only warns");
        send(&mut ide, [Message::CloseWindow]);
        assert_eq!(ide.window_names(), ["e.rs", "g.rs"]);
        assert_eq!(ide.path.as_deref(), Some(paths[2].as_path()), "the next window becomes active");
        send(&mut ide, [Message::CloseWindow, Message::CloseWindow]);
        assert_eq!(ide.window_names(), ["NONAME.RS"], "the last window turns into an empty one");
    }

    #[test]
    fn exit_warns_about_background_windows() {
        let paths = files(&["h.rs", "i.rs"]);
        let mut ide = Ide::default();
        send(&mut ide, [Message::Opened(Some(paths[0].clone())), Message::Edit(Action::Edit(Edit::Insert('x')))]);
        send(&mut ide, [Message::Opened(Some(paths[1].clone())), Message::Exit]);
        assert!(ide.status.starts_with("Unsaved changes"), "{}", ide.status);
        assert_eq!(ide.confirm, Some(Discard::Exit));
    }

    #[test]
    fn window_keys_and_menu() {
        let alt = Modifiers::ALT;
        let physical = key::Physical::Unidentified(key::NativeCode::Unidentified);
        let ch = |c: &str| Key::Character(c.into());
        assert!(matches!(shortcut(&ch("2"), physical, alt), Some(Message::SelectWindow(1))));
        assert!(matches!(shortcut(&ch("0"), physical, alt), Some(Message::OpenMenu(Menu::Window))));
        let f = |n| Key::Named(n);
        assert!(matches!(shortcut(&f(key::Named::F6), physical, Modifiers::empty()), Some(Message::NextWindow)));
        assert!(matches!(shortcut(&f(key::Named::F6), physical, Modifiers::SHIFT), Some(Message::PrevWindow)));
        assert!(matches!(shortcut(&f(key::Named::F3), physical, alt), Some(Message::CloseWindow)));

        let paths = files(&["j.rs", "k.rs"]);
        let mut ide = Ide::default();
        send(&mut ide, [Message::Opened(Some(paths[0].clone())), Message::Opened(Some(paths[1].clone()))]);
        send(&mut ide, [Message::OpenMenu(Menu::Window), Message::Nav(NavKey::Char('1'))]);
        assert_eq!(ide.active, 0, "digit hotkey picks a window from the list");
    }
}

#[cfg(test)]
mod syntax_check_tests {
    use super::*;

    fn rust_file(name: &str, text: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("check-ui-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    fn ide_with(path: &Path) -> Ide {
        let mut ide = Ide::default();
        ide.open_path(path);
        ide
    }

    fn result(success: bool, diagnostics: Vec<Diagnostic>) -> BuildResult {
        BuildResult { success, diagnostics, target: None }
    }

    fn diagnostic(file: &Path, level: &str, line: usize) -> Diagnostic {
        Diagnostic {
            file: Some(file.to_path_buf()),
            line,
            column: 5,
            level: level.into(),
            message: "message".into(),
        }
    }

    #[test]
    fn keys_alt_f9_checks_f9_compiles_ctrl_f9_runs() {
        let physical = key::Physical::Unidentified(key::NativeCode::Unidentified);
        let f9 = Key::Named(key::Named::F9);
        assert!(matches!(shortcut(&f9, physical, Modifiers::ALT), Some(Message::SyntaxCheck)));
        assert!(matches!(shortcut(&f9, physical, Modifiers::empty()), Some(Message::Compile)));
        assert!(matches!(shortcut(&f9, physical, Modifiers::CTRL), Some(Message::Run)));
    }

    #[test]
    fn compile_menu_has_syntax_check_with_its_key() {
        let items = menu_items(Menu::Compile, Toggles::default(), &[], 0);
        let check = items.iter().find(|i| i.label == "Syntax check").expect("menu item");
        assert_eq!((check.hotkey, check.keys), ('s', "Alt+F9"));
        assert!(matches!(check.message, Some(Message::SyntaxCheck)));
        assert!(SHORTCUTS.iter().any(|(_, keys, _)| *keys == "Alt+F9"));
        assert!(!SHORTCUTS.iter().any(|(_, keys, _)| keys.contains("F9 / Alt+F9")), "old help line is gone");
    }

    #[test]
    fn running_the_command_starts_a_check_not_a_build() {
        let path = rust_file("start.rs", "fn main() {}\n");
        let mut ide = ide_with(&path);
        let _ = ide.update(Message::SyntaxCheck);
        assert!(ide.building);
        assert!(ide.status.starts_with("Checking"), "{}", ide.status);

        let mut ide = ide_with(&path);
        let _ = ide.update(Message::Compile);
        assert!(ide.status.starts_with("Compiling"), "{}", ide.status);
    }

    #[test]
    fn check_results_set_the_status_and_jump_to_errors() {
        let path = rust_file("results.rs", "fn main() {\n    let x: i32 = \"no\";\n}\n");
        let mut ide = ide_with(&path);

        let _ = ide.finish_build(result(true, Vec::new()), AfterSave::Check, true);
        assert_eq!(ide.status, "No syntax errors");

        let _ = ide.finish_build(result(true, vec![diagnostic(&path, "warning", 1)]), AfterSave::Check, true);
        assert_eq!(ide.status, "No syntax errors, 1 warning(s)");
        assert_eq!(ide.panel_items().len(), 1, "warnings are listed in the Messages panel");

        let _ = ide.finish_build(result(false, vec![diagnostic(&path, "error", 2)]), AfterSave::Check, true);
        assert_eq!(ide.status, "Syntax check failed: 1 error(s), 0 warning(s)");
        assert_eq!(ide.content.cursor().position.line, 1, "jumps to the first error");
    }

    #[test]
    fn a_failed_check_before_compile_says_nothing_was_built() {
        let path = rust_file("before.rs", "fn main() {\n    let x: i32 = \"no\";\n}\n");
        let mut ide = ide_with(&path);
        let _ = ide.finish_build(result(false, vec![diagnostic(&path, "error", 2)]), AfterSave::Run, true);
        assert_eq!(ide.status, "Not built: syntax errors: 1 error(s), 0 warning(s)");
        assert!(ide.output.screen.is_empty(), "nothing ran");
    }

    #[test]
    fn check_before_stops_broken_code_and_lets_good_code_build() {
        let bad = rust_file("gate_bad.rs", "fn main() {\n    let x: i32 = \"no\";\n}\n");
        let (result, checked_only) = run_build(&bad, AfterSave::Run, true);
        assert!(!result.success && checked_only, "the check stops the build");
        assert!(result.target.is_none());

        let good = rust_file("gate_good.rs", "fn main() { println!(\"ok\"); }\n");
        let (result, checked_only) = run_build(&good, AfterSave::Run, true);
        assert!(result.success && !checked_only, "good code goes on to the build");
        assert!(result.target.is_some());

        let (result, checked_only) = run_build(&good, AfterSave::Check, false);
        assert!(result.success && checked_only);
        assert!(result.target.is_none(), "Alt+F9 never gives something to run");

        // With the setting off, a build runs by itself and reports its own errors.
        let (result, checked_only) = run_build(&bad, AfterSave::Compile, false);
        assert!(!result.success && !checked_only);
    }

    #[test]
    fn options_menu_shows_the_setting() {
        let on = Toggles { vim: false, check_before: true };
        let items = menu_items(Menu::Options, on, &[], 0);
        let item = items.iter().find(|i| i.label.starts_with("Check before run")).expect("menu item");
        assert!(item.label.ends_with("[x]") && item.hotkey == 'c');
        let items = menu_items(Menu::Options, Toggles::default(), &[], 0);
        assert!(items.iter().any(|i| i.label.starts_with("Check before run") && i.label.ends_with("[ ]")));
    }
}
