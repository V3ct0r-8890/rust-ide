//! rust-ide: a Turbo Pascalâ€“style IDE for Rust and Python (iced GUI front end).

// Release builds are GUI apps on Windows: no console window when started.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cursor;

use rust_ide::{dap, deps, filetype, highlight, lsp, output, platform, runner, search, templates, vim};

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::futures::channel::mpsc;
use iced::keyboard::{self, Key, Modifiers, key};
use iced::widget::text_editor::{Action, Binding, Cursor, Edit, KeyPress, Position};
use iced::widget::{
    Space, button, checkbox, column, container, markdown, mouse_area, opaque, row, scrollable, stack, text,
    text_editor, text_input,
};
use iced::{Background, Color, Element, Font, Length, Padding, Size, Subscription, Task, Theme, window};

use highlight::{Lang, Mark, MarkKind};
use platform::Os;
use runner::{BuildResult, Diagnostic, RunTarget};

/// The palette: Turbo Pascal colors, or grays with the monochrome theme (Options > Mono theme).
fn palette(turbo: (f32, f32, f32), mono: f32) -> Color {
    if platform::mono() { Color::from_rgb(mono, mono, mono) } else { Color::from_rgb(turbo.0, turbo.1, turbo.2) }
}

/// Editor and title background.
fn turbo_blue() -> Color {
    palette((0.0, 0.0, 0.66), 0.0)
}
/// Selected rows, the Messages panel.
fn turbo_cyan() -> Color {
    palette((0.0, 0.66, 0.66), 0.90)
}
/// Menus, dialogs, the status bar.
fn turbo_gray() -> Color {
    palette((0.75, 0.75, 0.75), 0.62)
}
fn turbo_dark_gray() -> Color {
    palette((0.33, 0.33, 0.33), 0.22)
}
/// Buttons and the chosen row.
fn turbo_green() -> Color {
    palette((0.0, 0.66, 0.0), 0.45)
}
/// Editor text and the block cursor.
fn turbo_yellow() -> Color {
    palette((1.0, 1.0, 0.33), 1.0)
}
fn turbo_red() -> Color {
    palette((0.66, 0.0, 0.0), 1.0)
}
const SCREEN_TEXT: Color = Color::from_rgb(0.75, 0.75, 0.75);

const MENU_TITLES: [&str; 10] = [
    "File", "Edit", "Search", "Run", "Compile", "Debug", "Tools", "Options", "Window", "Help",
];
const MENU_TITLE_WIDTH: f32 = 84.0;
const MENU_BAR_HEIGHT: f32 = 26.0;
static TWO_LINE_STATUS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The status bar: one line, or two lines of smaller text.
fn status_bar_height() -> f32 {
    if TWO_LINE_STATUS.load(std::sync::atomic::Ordering::Relaxed) { 40.0 } else { 24.0 }
}
const MESSAGES_HEIGHT: f32 = 140.0;
const COMPLETION_ROWS: usize = 10;
const OUTPUT_INPUT: &str = "output-input";
const SEARCH_INPUT: &str = "search-input";
const EDITOR: &str = "editor";
const HELP_SCROLL: &str = "help-scroll";
const PREVIEW_SCROLL: &str = "preview-scroll";
const DEPS_SCROLL: &str = "deps-scroll";

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
    ("File", "Alt+F N / A", "New window (asks for the language) / Save as (File menu)"),
    ("File", "Alt+F P", "New project: Rust or Python, size Simple/Low/Medium/Complex, a name and a folder"),
    ("File", "Tab / Up / Down", "In the New project dialog: next / previous field; Left/Right or letters change a choice; Enter creates"),
    ("File", "R P C T M or 1-5", "In the language list: Rust, Python, C#, Text, Markdown (Up/Down, Enter, Esc work too)"),
    ("File", "Alt+X", "Exit (press twice to discard changes)"),
    ("Window", "F6 / Shift+F6", "Next / previous window"),
    ("Window", "Alt+1 ... Alt+9", "Go to window 1-9"),
    ("Window", "Alt+0", "Window list (Window menu)"),
    ("Window", "Ctrl+F3", "Close the window (press twice to discard changes)"),
    ("Window", "Alt+F6 / Alt+P", "Markdown preview of a .md file: beside it, in a tab or in a window (Alt+W I chooses)"),
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
    ("Tools", "Alt+T D", "Dependencies: Up/Down/Tab (also reaches the buttons), I install, C copy, G guide, R re-check, S startup check"),
    ("Options", "Alt+O S", "Settings dialog: preview place, window style, Vim, check before run, line numbers, font size, dependencies"),
    ("Tools", "Alt+T G", "General environment: proxy, PATH ... for everything the IDE starts (NAME=value file)"),
    ("Tools", "Alt+T P", "Project environment: variables for this project (.rust-ide/env, overrides the general ones)"),
    ("Tools", "Alt+T S", "Show environment: the general and project variables that apply now"),
    ("Tools", "Alt+T V", "Python .venv setup: create .venv in the project and install requirements.txt (or the project)"),
    ("Tools", "Alt+T R", "Rust toolchain: pick one of the installed toolchains for this project (Up/Down, Enter, O other name)"),
    ("Options", "Alt+O V", "Vim mode on / off"),
    ("Options", "Alt+O T", "Mono theme on / off: black, white and grays instead of the Turbo Pascal colors"),
    ("Options", "Alt+O W", "Two-line status bar on / off: message and position on top, keys below, in smaller text"),
    ("Options", "Alt+O C", "Check the syntax before compile / run on / off"),
    ("Options", "Ctrl+= / Ctrl+- / Ctrl+0", "Editor font larger / smaller / default size (also Alt+O L / M / D)"),
    ("Window", "Alt+W L / H / T", "Window style: legacy windows / horizontal tabs / vertical tabs"),
    ("Help", "F1", "This list (Up/Down, Page Up/Down scroll; Esc or F1 closes)"),
    ("Vim", "i a o O, Esc", "Insert mode, back to Normal"),
    ("Vim", "h j k l w b e 0 ^ $ gg G", "Motions (with counts)"),
    ("Vim", "d c y > < + motion", "Operators; dd yy cc >> << for lines"),
    ("Vim", "f t F T + char, ; ,", "Find a character on the line; repeat / reverse"),
    ("Vim", "iw aw i\" a\" i( a( i{ a{ i[ i<", "Text objects after d c y (and in Visual mode)"),
    ("Vim", "x X D C Y p P J r u Ctrl+R", "Editing"),
    ("Vim", ".", "Repeat the last change"),
    ("Vim", "m{a-z}  '{a-z}  `{a-z}", "Set a mark; jump to its line / exact place"),
    ("Vim", "v V o, /text, n N", "Visual and Visual-line mode (o swaps ends), search"),
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

Turbo Pascal-style IDE for Rust, Python and C#.

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

    // A daemon (not an application) so that the Markdown preview can open its own window.
    iced::daemon(Ide::new, Ide::update, Ide::view)
        .title(Ide::title)
        .theme(|_: &Ide, _: window::Id| Theme::Dark)
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
    Tools,
    Options,
    Window,
    Help,
}

const MENUS: [Menu; 10] = [
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
    /// Shift+Tab.
    BackTab,
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

/// How open files (windows) are shown and chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum WindowStyle {
    /// Turbo Pascal: a numbered title line; windows are chosen with F6, Alt+1-9
    /// and the Window menu.
    Legacy,
    /// A strip of tabs under the menu bar.
    #[default]
    Horizontal,
    /// A list of tabs on the left of the editor.
    Vertical,
}

impl WindowStyle {
    /// The value kept in the settings file.
    fn setting(self) -> &'static str {
        match self {
            WindowStyle::Legacy => "legacy",
            WindowStyle::Horizontal => "horizontal",
            WindowStyle::Vertical => "vertical",
        }
    }

    fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("legacy") => WindowStyle::Legacy,
            Some("vertical") => WindowStyle::Vertical,
            _ => WindowStyle::Horizontal,
        }
    }

    fn label(self) -> &'static str {
        match self {
            WindowStyle::Legacy => "Legacy windows",
            WindowStyle::Horizontal => "Horizontal tabs",
            WindowStyle::Vertical => "Vertical tabs",
        }
    }
}

/// Where the Markdown preview is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PreviewMode {
    /// Next to the editor, following what is typed.
    #[default]
    Side,
    /// In a tab of its own (refreshed when the tab is selected).
    Tab,
    /// In a separate window, following what is typed.
    Window,
}

impl PreviewMode {
    fn setting(self) -> &'static str {
        match self {
            PreviewMode::Side => "side",
            PreviewMode::Tab => "tab",
            PreviewMode::Window => "window",
        }
    }

    fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("tab") => PreviewMode::Tab,
            Some("window") => PreviewMode::Window,
            _ => PreviewMode::Side,
        }
    }

    fn label(self) -> &'static str {
        match self {
            PreviewMode::Side => "side by side",
            PreviewMode::Tab => "a new tab",
            PreviewMode::Window => "a new window",
        }
    }

    fn next(self) -> Self {
        match self {
            PreviewMode::Side => PreviewMode::Tab,
            PreviewMode::Tab => PreviewMode::Window,
            PreviewMode::Window => PreviewMode::Side,
        }
    }
}

/// Identifies a text window while it moves between the editor fields and the
/// background list. A default value is a fresh, never used number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BufferId(u64);

impl Default for BufferId {
    fn default() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        BufferId(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

/// A window that shows the rendered Markdown of another window.
#[derive(Debug, Clone)]
struct PreviewTab {
    source: BufferId,
    /// The source's file name.
    title: String,
    /// Where relative links are resolved from.
    dir: Option<PathBuf>,
}

/// Settings shown as [x] / [ ] in the Options menu.
#[derive(Debug, Clone, Copy, Default)]
struct Toggles {
    vim: bool,
    check_before: bool,
    line_numbers: bool,
    mono: bool,
    two_line_status: bool,
    window_style: WindowStyle,
    preview: bool,
    preview_mode: PreviewMode,
}

/// Text size of the editor, the preview and the user screen, in points.
const FONT_DEFAULT: i16 = 16;
const FONT_MIN: i16 = 10;
const FONT_MAX: i16 = 32;

fn menu_items(menu: Menu, toggles: Toggles, windows: &[String], active: usize) -> Vec<MenuItem> {
    match menu {
        Menu::File => vec![
            item("New", 'n', "", Message::New),
            item("New project...", 'p', "", Message::OpenProject),
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
        // Setting up the machine and the project: tools, environment, toolchains.
        Menu::Tools => vec![
            item("Dependencies...", 'd', "", Message::ShowDeps),
            item("General environment...", 'g', "", Message::EditEnv(true)),
            item("Project environment...", 'p', "", Message::EditEnv(false)),
            item("Show environment", 's', "", Message::ShowEnv),
            item("Python .venv setup", 'v', "", Message::PythonSetup),
            item("Rust toolchain...", 'r', "", Message::OpenToolchains),
        ],
        // How the editor behaves and looks.
        Menu::Options => {
            let on = |flag: bool, label: &str| format!("{label:<18}{}", if flag { "[x]" } else { "[ ]" });
            vec![
                item("Settings...", 's', "", Message::OpenSettings),
                item(on(toggles.vim, "Vim mode"), 'v', "", Message::ToggleVim),
                item(on(toggles.check_before, "Check before run"), 'c', "", Message::ToggleCheckBefore),
                item(on(toggles.line_numbers, "Line numbers"), 'n', "", Message::ToggleLineNumbers),
                item(on(toggles.mono, "Mono theme"), 't', "", Message::ToggleMono),
                item(on(toggles.two_line_status, "Two-line status"), 'w', "", Message::ToggleStatusLines),
                item("Font larger", 'l', "Ctrl+=", Message::FontLarger),
                item("Font smaller", 'm', "Ctrl+-", Message::FontSmaller),
                item("Font default", 'd', "Ctrl+0", Message::FontDefault),
            ]
        }
        Menu::Window => {
            const ALT_KEYS: [&str; 9] = ["Alt+1", "Alt+2", "Alt+3", "Alt+4", "Alt+5", "Alt+6", "Alt+7", "Alt+8", "Alt+9"];
            let mut items = vec![
                item("Next", 'n', "F6", Message::NextWindow),
                item("Previous", 'p', "Shift+F6", Message::PrevWindow),
                item("Close", 'c', "Ctrl+F3", Message::CloseWindow),
                item(
                    if toggles.preview { "Markdown preview  [x]" } else { "Markdown preview  [ ]" },
                    'm',
                    "Alt+F6",
                    Message::TogglePreview,
                ),
                item(format!("Preview in: {}", toggles.preview_mode.label()), 'i', "", Message::CyclePreviewMode),
            ];
            // How windows are shown: pick one.
            for (style, hotkey) in [
                (WindowStyle::Legacy, 'l'),
                (WindowStyle::Horizontal, 'h'),
                (WindowStyle::Vertical, 't'),
            ] {
                let mark = if toggles.window_style == style { "(*)" } else { "( )" };
                items.push(item(format!("{:<19}{mark}", style.label()), hotkey, "", Message::WindowStyle(style)));
            }
            for (index, name) in windows.iter().enumerate() {
                let mark = if index == active { ">" } else { " " };
                let hotkey = char::from_digit((index as u32 + 1) % 10, 10).unwrap_or(' ');
                let keys = ALT_KEYS.get(index).copied().unwrap_or("");
                items.push(item(format!("{}{mark}{name}", index + 1), hotkey, keys, Message::SelectWindow(index)));
            }
            items
        }        Menu::Help => vec![
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
        Key::Named(Named::Tab) if modifiers.shift() => Some(NavKey::BackTab),
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
    /// Tab focus on the bottom row instead of the list: 0 startup check, 1 guide, 2 re-check, 3 close.
    button: Option<usize>,
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
    /// Font size now, in points (0 = not set yet; see `font_size`).
    font_points: i16,
    /// Vim emulation, when enabled in Options.
    vim: Option<vim::Vim>,
    /// Run the syntax check first for compile and run; stop on errors.
    check_before: bool,
    /// Line numbers in a column left of the editor.
    line_numbers: bool,
    /// Status bar on two lines with smaller text (Options > Two-line status).
    two_line_status: bool,
    /// Type (extension) chosen for the active window while it has no file name.
    untitled_ext: Option<&'static str>,
    language: Option<LanguageDialog>,
    settings: Option<SettingsDialog>,
    /// Options > Rust toolchain...: the installed toolchains to pick from.
    toolchains: Option<ToolchainDialog>,
    project: Option<ProjectDialog>,
    window_style: WindowStyle,
    /// Settings changes are written to the settings file (true in the real program).
    persist_settings: bool,
    /// Markdown preview next to the editor (shown for Markdown windows only).
    preview: bool,
    preview_mode: PreviewMode,
    /// The OS window of the program, and of the preview when it has its own.
    main_window: Option<window::Id>,
    preview_window: Option<window::Id>,
    /// The identity of the active text window.
    buffer_id: BufferId,
    /// Set when the active window is a preview tab.
    preview_tab: Option<PreviewTab>,
    /// Where the links of the shown preview are resolved from.
    preview_dir: Option<PathBuf>,
    preview_content: markdown::Content,
    /// The text `preview_content` was made from.
    preview_source: String,
    /// The cursor line the preview was last scrolled to.
    preview_line: usize,
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
    /// The "Rust toolchain" input (Options menu).
    Toolchain,
}

struct SearchBar {
    mode: SearchMode,
    query: String,
    replacement: String,
    case: bool,
}

/// Options > Rust toolchain...: one row per choice, then "Other name...".
struct ToolchainDialog {
    choices: Vec<rust_ide::env::ToolchainChoice>,
    selected: usize,
}

/// Options > Settings...: the main options in one dialog.
struct SettingsDialog {
    selected: usize,
}

/// Rows of the settings dialog (the last one closes it).
const SETTINGS_ROWS: usize = 10;

/// File > New project...: language, size, name and folder of a template project.
struct ProjectDialog {
    lang: templates::Lang,
    size: templates::Size,
    name: String,
    folder: String,
    /// The focused row: see `PROJECT_*`.
    row: usize,
    /// Why the last Create failed.
    error: String,
}

const PROJECT_LANG: usize = 0;
const PROJECT_SIZE: usize = 1;
const PROJECT_NAME: usize = 2;
const PROJECT_FOLDER: usize = 3;
const PROJECT_CREATE: usize = 4;
const PROJECT_ROWS: usize = 6;
const PROJECT_NAME_INPUT: &str = "project-name";
const PROJECT_FOLDER_INPUT: &str = "project-folder";

/// The "choose a language" list shown at start-up and by File > New.
struct LanguageDialog {
    selected: usize,
    /// Shown because no file was given on the command line; Esc then gives a
    /// plain Rust window instead of cancelling.
    startup: bool,
}

/// A file open in a window that is not the active one. The active window's
/// state lives directly in `Ide` (content, path, modified, undo, ...).
#[derive(Default)]
struct Buffer {
    id: BufferId,
    preview: Option<PreviewTab>,
    content: text_editor::Content,
    path: Option<PathBuf>,
    /// Type chosen for a window that has no file name yet.
    ext: Option<&'static str>,
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
    ToggleLineNumbers,
    ToggleMono,
    ToggleStatusLines,
    SyntaxCheck,
    NewFileType(usize),
    WindowStyle(WindowStyle),
    TogglePreview,
    CyclePreviewMode,
    OpenSettings,
    /// A settings row was used: the row, and -1 / +1 for Left / Right, 0 for Enter or a click.
    SettingsAct(usize, i8),
    OpenProject,
    EditEnv(bool),
    PythonSetup,
    PythonSetupDone(Result<String, String>),
    ShowEnv,
    ProjectName(String),
    ProjectFolder(String),
    /// A row of the New project dialog was clicked: the row, and -1 / +1 for Left / Right.
    ProjectAct(usize, i8),
    ProjectCreate,
    WindowOpened,
    WindowClosed(window::Id),
    FontLarger,
    FontSmaller,
    FontDefault,
    PreviewLink(String),
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
    SetToolchain,
    OpenToolchains,
    ToolchainsLoaded(Vec<String>),
    /// Row of the toolchain dialog: a click or Enter on it.
    ToolchainPick(usize),
    ClearWatches,
    Debug(u64, dap::Event),
}

impl Ide {
    /// Starts empty, or with the file given on the command line, and checks
    /// the external tools in the background.
    fn new() -> (Self, Task<Message>) {
        let mut ide = Self::default();
        ide.persist_settings = true;
        let os = Os::detect();
        ide.os = Some(os);
        ide.check_before = platform::read_setting("check_before").as_deref() == Some("true");
        ide.line_numbers = platform::read_setting("line_numbers").as_deref() == Some("true");
        platform::set_mono(platform::read_setting("theme").as_deref() == Some("mono"));
        ide.set_status_lines(platform::read_setting("status_lines").as_deref() == Some("2"));
        ide.window_style = WindowStyle::from_setting(platform::read_setting("window_style").as_deref());
        ide.font_points = platform::read_setting("font_size")
            .and_then(|v| v.trim().parse::<i16>().ok())
            .map_or(0, |v| v.clamp(FONT_MIN, FONT_MAX));
        if platform::read_setting("vim").as_deref() == Some("true") {
            ide.vim = Some(vim::Vim::default());
        }
        ide.preview_mode = PreviewMode::from_setting(platform::read_setting("preview_mode").as_deref());
        ide.check_at_startup =
            platform::read_setting("check_dependencies").as_deref() != Some("false");
        // Each file on the command line opens in its own window.
        let files: Vec<PathBuf> = std::env::args_os()
            .skip(1)
            .filter(|a| !a.to_string_lossy().starts_with("--"))
            .map(PathBuf::from)
            .collect();
        let no_files = files.is_empty();
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
        // Started without a file: bring back the last session, else ask which language to use.
        if no_files {
            let session = (platform::read_setting("restore_session").as_deref() != Some("false"))
                .then(|| std::env::current_dir().ok().and_then(|cwd| rust_ide::session::startup(&cwd)))
                .flatten();
            match session {
                Some(session) => ide.restore_session(&session),
                None => ide.language = Some(LanguageDialog { selected: filetype::last_choice(), startup: true }),
            }
        }
        let check = if ide.check_at_startup {
            Task::perform(platform::blocking(move || deps::check(os)), |list| Message::DepsChecked(list, true))
        } else {
            Task::none()
        };
        // Like Turbo Pascal, start with the cursor in the editor, ready to type.
        let (main, open_main) = window::open(window::Settings::default());
        ide.main_window = Some(main);
        (ide, Task::batch([open_main.map(|_| Message::WindowOpened), check, iced::widget::operation::focus(EDITOR)]))
    }

    /// Saves a setting for next time, but only in the real program: `Ide::default()`
    /// (used by the tests) never touches a settings file unless a test turns
    /// `persist_settings` on.
    /// Where the Open and Save As dialogs start: the active file's folder, else the last one used.
    fn dialog_dir(&self) -> Option<PathBuf> {
        let own = self.path.as_deref().and_then(Path::parent).filter(|d| d.is_dir()).map(Path::to_path_buf);
        own.or_else(platform::last_dir)
    }

    fn remember_dir(&mut self, file: &Path) {
        if self.persist_settings {
            platform::remember_dir(file);
        }
    }

    fn save_setting(&mut self, key: &str, value: &str) {
        if !self.persist_settings {
            return;
        }
        if let Err(e) = platform::write_setting(key, value) {
            self.status = format!("Cannot save settings: {e}");
        }
    }

    /// The font size in points, always inside the allowed range.
    fn font_size(&self) -> i16 {
        if self.font_points == 0 { FONT_DEFAULT } else { self.font_points.clamp(FONT_MIN, FONT_MAX) }
    }

    fn set_font_size(&mut self, points: i16) -> Task<Message> {
        let points = points.clamp(FONT_MIN, FONT_MAX);
        let limit = if points == self.font_size() && points != FONT_DEFAULT {
            if points == FONT_MAX { " (largest)" } else { " (smallest)" }
        } else {
            ""
        };
        self.font_points = points;
        self.status = format!("Font size {points}{limit}");
        self.save_setting("font_size", &points.to_string());
        iced::widget::operation::focus(EDITOR)
    }

    fn toggles(&self) -> Toggles {
        Toggles {
            vim: self.vim.is_some(),
            check_before: self.check_before,
            line_numbers: self.line_numbers,
            mono: platform::mono(),
            two_line_status: self.two_line_status,
            window_style: self.window_style,
            preview: self.preview_shown(),
            preview_mode: self.preview_mode,
        }
    }

    fn title(&self, window: window::Id) -> String {
        if Some(window) == self.preview_window {
            return "Markdown preview - Rust IDE".into();
        }
        if let Some(tab) = &self.preview_tab {
            return format!("Preview {} - Rust IDE", tab.title);
        }
        let name = self
            .path
            .as_deref()
            .and_then(Path::file_name)
            .map_or(filetype::untitled_name(self.untitled_ext), |n| n.to_string_lossy().into_owned());
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
        let closed = window::close_events().map(Message::WindowClosed);
        Subscription::batch([keys, escape, closed, Subscription::run(lsp::connect).map(Message::Lsp)])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.update_inner(message);
        Task::batch([task, self.sync_preview()])
    }

    fn update_inner(&mut self, message: Message) -> Task<Message> {
        // A preview tab has no text to edit, save or build.
        if self.preview_tab.is_some()
            && matches!(
                message,
                Message::Edit(_)
                    | Message::Vim(_)
                    | Message::Save
                    | Message::SaveAs
                    | Message::Compile
                    | Message::Run
                    | Message::RunExternal
                    | Message::SyntaxCheck
                    | Message::Undo
                    | Message::Redo
                    | Message::OpenSearch(_)
                    | Message::ReplaceOne
                    | Message::ReplaceAll
                    | Message::ToggleBreakpoint
                    | Message::DebugRun
                    | Message::DebugStep(_)
                    | Message::GoToCursor
                    | Message::TriggerCompletion
            )
        {
            self.status = "This is a preview tab; press Alt+F6 to go back to the file".into();
            return Task::none();
        }
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
            Message::New => self.ask_language(false),
            Message::NewFileType(index) => self.choose_language(index),
            Message::FontLarger => self.set_font_size(self.font_size() + 1),
            Message::FontSmaller => self.set_font_size(self.font_size() - 1),
            Message::FontDefault => self.set_font_size(FONT_DEFAULT),
            Message::TogglePreview => self.toggle_preview(),
            Message::CyclePreviewMode => self.set_preview_mode(self.preview_mode.next()),
            Message::OpenSettings => {
                self.settings = Some(SettingsDialog { selected: 0 });
                Task::none()
            }
            Message::OpenProject => self.open_project_dialog(),
            Message::EditEnv(general) => {
                let made = if general {
                    rust_ide::env::ensure_general_file()
                } else {
                    rust_ide::env::ensure_project_file(&self.project_dir())
                };
                match made {
                    Ok(path) => {
                        self.open_path(&path);
                    }
                    Err(e) => self.status = format!("Cannot create the environment file: {e}"),
                }
                iced::widget::operation::focus(EDITOR)
            }
            Message::PythonSetup => {
                let dir = self.project_dir();
                self.status = "Setting up the Python environment (.venv, requirements) ...".into();
                Task::perform(platform::blocking(move || rust_ide::env::python_setup(&dir)), Message::PythonSetupDone)
            }
            Message::PythonSetupDone(result) => {
                self.status = result.unwrap_or_else(|e| e);
                Task::none()
            }
            Message::ShowEnv => {
                let text = rust_ide::env::describe(Some(&self.project_dir()));
                self.open_window(Buffer::default());
                self.content = text_editor::Content::with_text(&text);
                self.path = None;
                self.untitled_ext = None;
                self.modified = false;
                self.undo.clear();
                self.redo.clear();
                self.set_live(Vec::new());
                self.status = "Environment for programs started by the IDE (listing; edit the files from Options)".into();
                iced::widget::operation::focus(EDITOR)
            }
            Message::ProjectName(name) => {
                if let Some(dialog) = &mut self.project {
                    dialog.name = name;
                    dialog.error.clear();
                }
                Task::none()
            }
            Message::ProjectFolder(folder) => {
                if let Some(dialog) = &mut self.project {
                    dialog.folder = folder;
                    dialog.error.clear();
                }
                Task::none()
            }
            Message::ProjectAct(row, dir) => self.project_act(row, dir),
            Message::ProjectCreate => self.project_create(),
            Message::SettingsAct(row, dir) => {
                if let Some(dialog) = &mut self.settings {
                    dialog.selected = row;
                }
                self.settings_act(row, dir)
            }
            Message::WindowOpened => Task::none(),
            Message::WindowClosed(id) => {
                if Some(id) == self.main_window {
                    self.exit_ide()
                } else {
                    if Some(id) == self.preview_window {
                        self.preview_window = None;
                        self.status = "Markdown preview window closed".into();
                    }
                    Task::none()
                }
            }
            Message::PreviewLink(url) => {
                self.open_link(&url);
                Task::none()
            }
            Message::WindowStyle(style) => {
                self.window_style = style;
                self.status = format!("Window style: {}", style.label().to_lowercase());
                self.save_setting("window_style", style.setting());
                // The layout changes, which makes the editor lose the keyboard focus.
                iced::widget::operation::focus(EDITOR)
            }
            Message::Open => {
                let mut dialog = rfd::AsyncFileDialog::new();
                if let Some(dir) = self.dialog_dir() {
                    dialog = dialog.set_directory(dir);
                }
                Task::perform(
                    dialog
                        .set_title("Open")
                        .add_filter("Source and text", &["rs", "py", "pyw", "cs", "txt", "md"])
                        .add_filter("All files", &["*"])
                        .pick_file(),
                    |file| Message::Opened(file.map(|f| f.path().to_path_buf())),
                )
            }
            Message::Opened(Some(path)) => {
                self.remember_dir(&path);
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
            Message::SaveAs => self.save_as(AfterSave::Nothing),
            Message::SaveAsPicked(Some(path), after) => {
                self.remember_dir(&path);
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
                self.exit_ide()
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
            Message::OpenToolchains => {
                self.status = "Looking for installed Rust toolchains ...".into();
                Task::perform(platform::blocking(rust_ide::env::installed_toolchains), Message::ToolchainsLoaded)
            }
            Message::ToolchainsLoaded(installed) => {
                let choices = rust_ide::env::toolchain_choices(&installed, &self.project_dir());
                let selected = choices.iter().position(|c| c.current).unwrap_or(0);
                self.toolchains = Some(ToolchainDialog { choices, selected });
                self.status = if installed.is_empty() { "rustup found no toolchains (is rustup installed?)".into() } else { String::new() };
                Task::none()
            }
            Message::ToolchainPick(row) => {
                let Some(dialog) = &mut self.toolchains else { return Task::none() };
                dialog.selected = row;
                match dialog.choices.get(row).cloned() {
                    Some(choice) => {
                        let name = (!choice.name.is_empty()).then_some(choice.name.as_str());
                        self.status = match rust_ide::env::set_rust_toolchain(&self.project_dir(), name) {
                            Ok(message) if choice.usable => message,
                            Ok(message) => format!("{message} - warning: {}", choice.note),
                            Err(e) => e,
                        };
                        self.toolchains = None;
                        iced::widget::operation::focus(EDITOR)
                    }
                    None => {
                        // The row after the list: type a name instead.
                        self.toolchains = None;
                        self.open_search(SearchMode::Toolchain)
                    }
                }
            }
            Message::SetToolchain => {
                let name = self.search.as_ref().map(|b| b.query.trim().to_string()).unwrap_or_default();
                let result = rust_ide::env::set_rust_toolchain(&self.project_dir(), (!name.is_empty()).then_some(name.as_str()));
                match result {
                    Ok(message) => {
                        self.status = message;
                        self.close_search()
                    }
                    Err(e) => {
                        self.status = e;
                        Task::none()
                    }
                }
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
                self.save_setting("check_before", if on { "true" } else { "false" });
                Task::none()
            }
            Message::ToggleMono => {
                platform::set_mono(!platform::mono());
                self.status = format!("Mono theme {}", if platform::mono() { "on" } else { "off" });
                self.save_setting("theme", if platform::mono() { "mono" } else { "turbo" });
                Task::none()
            }
            Message::ToggleStatusLines => {
                self.set_status_lines(!self.two_line_status);
                self.status = format!("Status bar on {} line(s)", if self.two_line_status { 2 } else { 1 });
                self.save_setting("status_lines", if self.two_line_status { "2" } else { "1" });
                iced::widget::operation::focus(EDITOR)
            }
            Message::ToggleLineNumbers => {
                self.line_numbers = !self.line_numbers;
                self.status = format!("Line numbers {}", if self.line_numbers { "on" } else { "off" });
                self.save_setting("line_numbers", if self.line_numbers { "true" } else { "false" });
                Task::none()
            }
            Message::ToggleVim => {
                self.vim = match self.vim {
                    Some(_) => None,
                    None => Some(vim::Vim::default()),
                };
                let on = self.vim.is_some();
                self.status = format!("Vim mode {}", if on { "on" } else { "off" });
                self.save_setting("vim", if on { "true" } else { "false" });
                Task::none()
            }
            Message::CompletionPick(index) => {
                self.accept_completion(index);
                Task::none()
            }
            Message::Escape => {
                if self.language.is_some() {
                    return self.close_language_dialog();
                }
                if self.settings.is_some() {
                    self.settings = None;
                    return iced::widget::operation::focus(EDITOR);
                }
                if self.toolchains.is_some() {
                    self.toolchains = None;
                    return iced::widget::operation::focus(EDITOR);
                }
                if self.project.is_some() {
                    self.project = None;
                    return iced::widget::operation::focus(EDITOR);
                }
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
                let button = self.deps.as_ref().and_then(|d| d.button);
                self.deps = Some(DepsDialog { list, selected, checking: false, installing: None, message, button });
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
                self.save_setting("check_dependencies", if on { "true" } else { "false" });
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
                    button: None,
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
            let lang = self.lang();
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
        let name = |path: &Option<PathBuf>, ext: Option<&'static str>, modified: bool, tab: &Option<PreviewTab>| {
            if let Some(tab) = tab {
                return format!("Preview {}", tab.title);
            }
            let file = path
                .as_deref()
                .and_then(Path::file_name)
                .map_or(filetype::untitled_name(ext), |n| n.to_string_lossy().into_owned());
            if modified { format!("{file}*") } else { file }
        };
        let mut names: Vec<String> = self.background.iter().map(|b| name(&b.path, b.ext, b.modified, &b.preview)).collect();
        names.insert(self.active, name(&self.path, self.untitled_ext, self.modified, &self.preview_tab));
        names
    }

    /// The language of the active window: from its file name, or from the
    /// type chosen for a new file.
    fn lang(&self) -> Lang {
        match &self.path {
            Some(path) => Lang::from_path(Some(path)),
            None => Lang::from_extension(self.untitled_ext),
        }
    }

    /// The name offered by Save As.
    fn suggested_name(&self) -> String {
        match self.path.as_deref().and_then(Path::file_name) {
            Some(name) => name.to_string_lossy().into_owned(),
            None => filetype::untitled_name(self.untitled_ext),
        }
    }

    fn save_as(&self, after: AfterSave) -> Task<Message> {
        let ext = match self.path.as_deref().and_then(Path::extension) {
            Some(ext) => ext.to_string_lossy().into_owned(),
            None => self.untitled_ext.unwrap_or(filetype::DEFAULT_EXT).to_string(),
        };
        save_as_dialog(after, self.suggested_name(), ext, self.dialog_dir())
    }

    // ----- New file: choose a language -----

    fn ask_language(&mut self, startup: bool) -> Task<Message> {
        self.language = Some(LanguageDialog { selected: filetype::last_choice(), startup });
        iced::widget::operation::focus(EDITOR)
    }

    fn choose_language(&mut self, index: usize) -> Task<Message> {
        let ext = filetype::ext_of(index);
        self.language = None;
        if self.persist_settings {
            filetype::remember(ext);
        }
        self.new_window_of(ext);
        iced::widget::operation::focus(EDITOR)
    }

    /// Esc: at start-up a plain Rust window, from File > New nothing.
    fn close_language_dialog(&mut self) -> Task<Message> {
        if let Some(dialog) = self.language.take()
            && dialog.startup
        {
            self.new_window_of(filetype::DEFAULT_EXT);
        }
        iced::widget::operation::focus(EDITOR)
    }

    /// A new untitled window of this type (an empty untitled one is reused).
    fn new_window_of(&mut self, ext: &'static str) {
        if self.is_blank_window() {
            self.untitled_ext = Some(ext);
        } else {
            self.open_window(Buffer { ext: Some(ext), ..Buffer::default() });
        }
        self.status = format!("New file ({})", filetype::untitled_name(Some(ext)));
    }

    fn language_key(&mut self, key: NavKey) -> Task<Message> {
        let Some(dialog) = &mut self.language else { return Task::none() };
        let count = filetype::FILE_TYPES.len();
        match key {
            NavKey::Up | NavKey::BackTab => dialog.selected = (dialog.selected + count - 1) % count,
            NavKey::Down | NavKey::Tab => dialog.selected = (dialog.selected + 1) % count,
            NavKey::Home => dialog.selected = 0,
            NavKey::End => dialog.selected = count - 1,
            NavKey::Enter => {
                let index = dialog.selected;
                return self.choose_language(index);
            }
            NavKey::Char(c) => {
                if let Some(index) = filetype::by_key(c) {
                    return self.choose_language(index);
                }
            }
            _ => {}
        }
        Task::none()
    }

    /// An untitled, unchanged, empty window that a file can simply replace.
    fn is_blank_window(&self) -> bool {
        self.path.is_none() && self.preview_tab.is_none() && !self.modified && self.content.text().trim().is_empty()
    }

    /// Moves the active window's state out of the editor fields.
    fn take_active(&mut self) -> Buffer {
        self.undo_group = false;
        Buffer {
            id: std::mem::take(&mut self.buffer_id),
            preview: self.preview_tab.take(),
            content: std::mem::take(&mut self.content),
            path: self.path.take(),
            ext: self.untitled_ext.take(),
            modified: std::mem::take(&mut self.modified),
            undo: std::mem::take(&mut self.undo),
            redo: std::mem::take(&mut self.redo),
            live: std::mem::take(&mut self.live),
            marks: std::mem::take(&mut self.marks),
        }
    }

    fn put_active(&mut self, buffer: Buffer) {
        self.buffer_id = buffer.id;
        self.preview_tab = buffer.preview;
        self.content = buffer.content;
        self.path = buffer.path;
        self.untitled_ext = buffer.ext;
        self.modified = buffer.modified;
        self.undo = buffer.undo;
        self.redo = buffer.redo;
        self.live = buffer.live;
        self.marks = buffer.marks;
        self.completion = None;
        self.message_index = None;
        self.refresh_search_marks();
        self.lsp_sync_doc();
        self.refresh_preview_tab();
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
    /// The folder the project environment belongs to: the open file's, else the working folder.
    fn project_dir(&self) -> PathBuf {
        self.path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    }

    /// "  Rust nightly" in the status bar while a Rust file is open.
    fn toolchain_label(&self) -> String {
        match self.path.as_deref() {
            Some(path) if path.extension().is_some_and(|e| e == "rs") => {
                format!("  {}", rust_ide::env::toolchain_label(&self.project_dir()))
            }
            _ => String::new(),
        }
    }

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
        self.untitled_ext = None;
        self.modified = false;
        self.undo.clear();
        self.redo.clear();
        self.set_live(Vec::new());
        self.lsp_sync_doc();
        self.status = format!("Opened {}", path.display());
        true
    }

    // ----- Sessions -----

    /// The open files with their cursors, in window order.
    fn session_snapshot(&self) -> rust_ide::session::Session {
        use rust_ide::session::{Entry, Session};
        let entry = |path: &Option<PathBuf>, content: &text_editor::Content, preview: bool| -> Option<Entry> {
            let path = path.clone().filter(|_| !preview)?;
            let at = content.cursor().position;
            let column = content.line(at.line).and_then(|l| l.text.get(..at.column).map(|s| s.chars().count())).unwrap_or(at.column);
            Some(Entry { path, line: at.line, column })
        };
        let mut session = Session::default();
        for index in 0..self.window_count() {
            let found = if index == self.active {
                entry(&self.path, &self.content, self.preview_tab.is_some())
            } else {
                let b = &self.background[if index > self.active { index - 1 } else { index }];
                entry(&b.path, &b.content, b.preview.is_some())
            };
            if let Some(found) = found {
                if index == self.active {
                    session.active = session.files.len();
                }
                session.files.push(found);
            }
        }
        session
    }

    /// Remembers the open files for the next start (the real program only).
    fn save_session(&self) {
        if !self.persist_settings {
            return;
        }
        let session = self.session_snapshot();
        if session.files.is_empty() {
            if let Ok(cwd) = std::env::current_dir() {
                rust_ide::session::forget(&rust_ide::env::project_root(&cwd));
            }
        } else {
            let _ = rust_ide::session::save(&session);
        }
    }

    fn exit_ide(&self) -> Task<Message> {
        self.save_session();
        iced::exit()
    }

    /// Opens the files of an earlier session, each with its cursor, on the window that was active.
    fn restore_session(&mut self, session: &rust_ide::session::Session) {
        let mut opened = 0;
        for entry in &session.files {
            if self.open_path(&entry.path) {
                opened += 1;
                let text = self.content.line(entry.line).map(|l| l.text.into_owned()).unwrap_or_default();
                let column = text.char_indices().nth(entry.column).map_or(text.len(), |(i, _)| i);
                self.place_clamped(entry.line, column);
            }
        }
        if let Some(active) = session.files.get(session.active) {
            if let Some(index) = self.window_index_of(&active.path) {
                let _ = self.select_window(index);
            }
        }
        self.status = format!("Restored the last session: {opened} file(s)");
    }

    /// The window position of the background list entry `position`.
    fn window_position(&self, position: usize) -> usize {
        // Background order skips the active window.
        if position >= self.active { position + 1 } else { position }
    }

    fn index_of_id(&self, id: BufferId) -> Option<usize> {
        if self.buffer_id == id {
            return Some(self.active);
        }
        self.background.iter().position(|b| b.id == id).map(|p| self.window_position(p))
    }

    /// The preview tab that shows window `source`, if one is open.
    fn find_preview_tab(&self, source: BufferId) -> Option<usize> {
        let position = self.background.iter().position(|b| b.preview.as_ref().is_some_and(|t| t.source == source))?;
        Some(self.window_position(position))
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
            Lang::CSharp => {
                let Some(project) = runner::find_csproj(&path) else {
                    self.status = "Debugging C# needs a project (.csproj); single files can only run (F5)".into();
                    return Task::none();
                };
                let Some(debugger) = dap::find_netcoredbg(&root) else {
                    self.status = "Debugging C# needs netcoredbg (see Tools > Dependencies)".into();
                    return Task::none();
                };
                let Some(dll) = dap::csharp_dll(&project) else {
                    self.status = "No Debug build of the project found - build it first (F9)".into();
                    return Task::none();
                };
                let dotnet = target.program.to_string_lossy().into_owned();
                dap::Launch {
                    adapter: dap::Adapter::NetCoreDbg,
                    command: vec![debugger.to_string_lossy().into_owned(), "--interpreter=vscode".into()],
                    arguments: serde_json::json!({
                        "program": dotnet,
                        "args": [dll],
                        "cwd": target.cwd,
                        "stopAtEntry": start == DebugStart::Entry,
                        "justMyCode": true,
                    }),
                    entry_function: None,
                    breakpoints,
                    temporary,
                }
            }
            Lang::Rust => {
                let Some((gdb, _)) = dap::find_gdb() else {
                    self.status = "Debugging Rust needs gdb 14 or newer (see Tools > Dependencies)".into();
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
                    self.status = "Debugging Python needs debugpy (see Tools > Dependencies)".into();
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
            button(text(format!(" {} {name} ", index + 1)).color(if active { Color::BLACK } else { turbo_dark_gray() }))
                .padding([1, 4])
                .on_press(Message::SelectWindow(index))
                .style(move |_, status| button::Style {
                    background: Some(Background::Color(
                        if active || matches!(status, button::Status::Hovered) { turbo_cyan() } else { turbo_gray() },
                    )),
                    ..button::Style::default()
                })
                .into()
        });
        container(row(tabs).spacing(2))
            .width(Length::Fill)
            .padding([1, 4])
            .style(|_| container::Style::default().background(turbo_gray()))
            .into()
    }

    // ----- Markdown preview -----

    /// A Markdown file: by its name, or the chosen type of an untitled window.
    fn is_markdown(&self) -> bool {
        let ext = match &self.path {
            Some(path) => path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase),
            None => self.untitled_ext.map(str::to_string),
        };
        matches!(ext.as_deref(), Some("md" | "markdown"))
    }

    /// The preview is drawn next to the editor.
    fn preview_visible(&self) -> bool {
        self.preview_mode == PreviewMode::Side && self.preview && self.is_markdown()
    }

    /// The active Markdown window is being previewed beside it or in the
    /// preview window, so the preview has to follow it.
    fn preview_live(&self) -> bool {
        self.is_markdown()
            && match self.preview_mode {
                PreviewMode::Side => self.preview,
                PreviewMode::Window => self.preview_window.is_some(),
                PreviewMode::Tab => false,
            }
    }

    /// For the [x] / [ ] of the Window menu.
    fn preview_shown(&self) -> bool {
        match self.preview_mode {
            PreviewMode::Side => self.preview,
            PreviewMode::Tab => self.preview_tab.is_some() || self.find_preview_tab(self.buffer_id).is_some(),
            PreviewMode::Window => self.preview_window.is_some(),
        }
    }

    fn set_preview_mode(&mut self, mode: PreviewMode) -> Task<Message> {
        self.preview_mode = mode;
        self.preview = false;
        self.status = format!("Markdown preview opens {}", mode.label());
        self.save_setting("preview_mode", mode.setting());
        let close = self.preview_window.take().map_or_else(Task::none, window::close);
        Task::batch([close, iced::widget::operation::focus(EDITOR)])
    }

    // ----- New project dialog -----

    fn open_project_dialog(&mut self) -> Task<Message> {
        self.project = Some(ProjectDialog {
            lang: templates::last_lang(),
            size: templates::last_size(),
            name: String::new(),
            folder: templates::default_folder(),
            row: PROJECT_LANG,
            error: String::new(),
        });
        // Arrow keys and letters must reach the dialog, not a text field.
        iced::widget::operation::focus(EDITOR)
    }

    /// Moves to a row and gives a text row the keyboard focus.
    fn project_focus(&mut self, row: usize) -> Task<Message> {
        let Some(dialog) = &mut self.project else { return Task::none() };
        dialog.row = row;
        match row {
            PROJECT_NAME => iced::widget::operation::focus(PROJECT_NAME_INPUT),
            PROJECT_FOLDER => iced::widget::operation::focus(PROJECT_FOLDER_INPUT),
            _ => iced::widget::operation::focus(EDITOR),
        }
    }

    fn project_key(&mut self, key: NavKey) -> Task<Message> {
        let Some(dialog) = &self.project else { return Task::none() };
        let row = dialog.row;
        let text_row = row == PROJECT_NAME || row == PROJECT_FOLDER;
        match key {
            NavKey::Up | NavKey::BackTab => self.project_focus((row + PROJECT_ROWS - 1) % PROJECT_ROWS),
            NavKey::Down | NavKey::Tab => self.project_focus((row + 1) % PROJECT_ROWS),
            // In a text field the keys edit the text (Enter creates, through on_submit).
            _ if text_row => Task::none(),
            NavKey::Left => self.project_act(row, -1),
            NavKey::Right => self.project_act(row, 1),
            NavKey::Enter | NavKey::Char(' ') => self.project_act(row, 0),
            NavKey::Char(c) => {
                match row {
                    PROJECT_LANG => {
                        if let Some(lang) = templates::LANGS.into_iter().find(|l| l.label().to_lowercase().starts_with(c)) {
                            if let Some(dialog) = &mut self.project {
                                dialog.lang = lang;
                            }
                        }
                    }
                    PROJECT_SIZE => {
                        if let Some(size) = templates::SIZES.into_iter().find(|s| s.label().to_lowercase().starts_with(c)) {
                            if let Some(dialog) = &mut self.project {
                                dialog.size = size;
                            }
                        }
                    }
                    _ => {}
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    /// Changes or activates a row: `dir` is -1 / +1 (Left / Right) or 0 (Enter, click).
    fn project_act(&mut self, row: usize, dir: i8) -> Task<Message> {
        let Some(dialog) = &mut self.project else { return Task::none() };
        let step = |len: usize, at: usize| if dir < 0 { (at + len - 1) % len } else { (at + 1) % len };
        match row {
            PROJECT_LANG => {
                let at = templates::LANGS.iter().position(|l| *l == dialog.lang).unwrap_or(0);
                dialog.lang = templates::LANGS[step(templates::LANGS.len(), at)];
                dialog.row = row;
                Task::none()
            }
            PROJECT_SIZE => {
                let at = templates::SIZES.iter().position(|s| *s == dialog.size).unwrap_or(0);
                dialog.size = templates::SIZES[step(templates::SIZES.len(), at)];
                dialog.row = row;
                Task::none()
            }
            PROJECT_NAME | PROJECT_FOLDER => self.project_focus(row),
            PROJECT_CREATE => self.project_create(),
            _ => {
                self.project = None;
                iced::widget::operation::focus(EDITOR)
            }
        }
    }

    fn project_create(&mut self) -> Task<Message> {
        let Some(dialog) = &mut self.project else { return Task::none() };
        let name = dialog.name.trim().to_string();
        let folder = PathBuf::from(dialog.folder.trim());
        match templates::create(dialog.lang, dialog.size, &name, &folder) {
            Err(error) => {
                dialog.error = error;
                Task::none()
            }
            Ok(created) => {
                let (lang, size, folder_text) = (dialog.lang, dialog.size, dialog.folder.trim().to_string());
                self.project = None;
                if self.persist_settings {
                    templates::remember(lang, size, &folder_text);
                }
                let opened = self.open_path(&created.main_file);
                if opened {
                    self.status = format!("Created {} {} project in {}", size.label(), lang.label(), created.dir.display());
                }
                iced::widget::operation::focus(EDITOR)
            }
        }
    }

    // ----- Settings dialog -----

    fn settings_key(&mut self, key: NavKey) -> Task<Message> {
        let Some(dialog) = &mut self.settings else { return Task::none() };
        let row = dialog.selected;
        match key {
            NavKey::Up | NavKey::BackTab => dialog.selected = (row + SETTINGS_ROWS - 1) % SETTINGS_ROWS,
            NavKey::Down | NavKey::Tab => dialog.selected = (row + 1) % SETTINGS_ROWS,
            NavKey::Home => dialog.selected = 0,
            NavKey::End => dialog.selected = SETTINGS_ROWS - 1,
            NavKey::Left => return self.settings_act(row, -1),
            NavKey::Right => return self.settings_act(row, 1),
            NavKey::Enter | NavKey::Char(' ') => return self.settings_act(row, 0),
            NavKey::Char(c) => {
                if let Some(index) = ['p', 'w', 'v', 'c', 'n', 'f', 'm', 's', 'd'].iter().position(|&k| k == c) {
                    dialog.selected = index;
                    return self.settings_act(index, 0);
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn toolchain_key(&mut self, key: NavKey) -> Task<Message> {
        let Some(dialog) = &mut self.toolchains else { return Task::none() };
        let rows = dialog.choices.len() + 1;
        match key {
            NavKey::Up | NavKey::BackTab => dialog.selected = (dialog.selected + rows - 1) % rows,
            NavKey::Down | NavKey::Tab => dialog.selected = (dialog.selected + 1) % rows,
            NavKey::Home => dialog.selected = 0,
            NavKey::End => dialog.selected = rows - 1,
            NavKey::Enter | NavKey::Char(' ') => {
                let row = dialog.selected;
                return self.update(Message::ToolchainPick(row));
            }
            NavKey::Char('o') => return self.update(Message::ToolchainPick(rows - 1)),
            _ => {}
        }
        Task::none()
    }

    /// Changes the setting of a row: `dir` is -1 / +1 (Left / Right) or 0 (Enter, click).
    fn settings_act(&mut self, row: usize, dir: i8) -> Task<Message> {
        match row {
            0 => {
                let mode = if dir < 0 { self.preview_mode.next().next() } else { self.preview_mode.next() };
                self.set_preview_mode(mode)
            }
            1 => {
                let next = match self.window_style {
                    WindowStyle::Legacy => WindowStyle::Horizontal,
                    WindowStyle::Horizontal => WindowStyle::Vertical,
                    WindowStyle::Vertical => WindowStyle::Legacy,
                };
                let style = if dir < 0 {
                    match self.window_style {
                        WindowStyle::Legacy => WindowStyle::Vertical,
                        WindowStyle::Horizontal => WindowStyle::Legacy,
                        WindowStyle::Vertical => WindowStyle::Horizontal,
                    }
                } else {
                    next
                };
                self.update(Message::WindowStyle(style))
            }
            2 => self.update(Message::ToggleVim),
            3 => self.update(Message::ToggleCheckBefore),
            4 => self.update(Message::ToggleLineNumbers),
            5 => self.update(if dir < 0 { Message::FontSmaller } else { Message::FontLarger }),
            6 => self.update(Message::ToggleMono),
            7 => self.update(Message::ToggleStatusLines),
            8 => {
                self.settings = None;
                self.update(Message::ShowDeps)
            }
            _ => {
                self.settings = None;
                iced::widget::operation::focus(EDITOR)
            }
        }
    }

    fn settings_dialog(&self, dialog: &SettingsDialog) -> Element<'static, Message> {
        let on = |flag: bool| if flag { "[x]" } else { "[ ]" }.to_string();
        let rows: [(&str, char, String); SETTINGS_ROWS] = [
            ("Markdown preview opens", 'p', self.preview_mode.label().to_string()),
            ("Window style", 'w', self.window_style.label().to_string()),
            ("Vim mode", 'v', on(self.vim.is_some())),
            ("Check before run", 'c', on(self.check_before)),
            ("Line numbers", 'n', on(self.line_numbers)),
            ("Font size", 'f', format!("< {} >", self.font_size())),
            ("Mono theme", 'm', on(platform::mono())),
            ("Two-line status", 's', on(self.two_line_status)),
            ("Dependencies...", 'd', String::new()),
            ("Close", '\0', "Esc".to_string()),
        ];
        let entries = rows.into_iter().enumerate().map(|(index, (label, hotkey, value))| {
            let chosen = index == dialog.selected;
            button(row![
                hotkey_label(label, hotkey),
                Space::new().width(Length::Fill),
                text(value).color(Color::BLACK),
            ])
            .width(Length::Fill)
            .padding([3, 10])
            .on_press(Message::SettingsAct(index, if index == 5 { 1 } else { 0 }))
            .style(move |_, status| button::Style {
                background: Some(Background::Color(
                    if chosen || matches!(status, button::Status::Hovered) { turbo_green() } else { turbo_gray() },
                )),
                ..button::Style::default()
            })
            .into()
        });
        let body = column![
            container(text(" Settings ").color(Color::WHITE))
                .width(Length::Fill)
                .padding(2)
                .style(|_| container::Style::default().background(turbo_blue())),
            column(entries).padding(8),
            container(text(" Up/Down  Left/Right or Enter changes  P W V C N F M S D  Esc closes").color(Color::BLACK).size(13))
                .padding(10),
        ];
        opaque(
            container(container(body).width(560).style(|_| container::Style {
                border: iced::Border::default().color(Color::BLACK).width(1),
                ..container::Style::default().background(turbo_gray())
            }))
            .center(Length::Fill),
        )
    }

    /// Alt+F6: shows or hides the preview where the Options say.
    fn toggle_preview(&mut self) -> Task<Message> {
        let focus = iced::widget::operation::focus(EDITOR);
        match self.preview_mode {
            PreviewMode::Side => {
                if !self.is_markdown() {
                    self.status = "The preview is for Markdown (.md) files".into();
                } else {
                    self.preview = !self.preview;
                    self.status = format!("Markdown preview {}", if self.preview { "on" } else { "off" });
                }
                // The layout changes, which makes the editor lose the keyboard focus.
                focus
            }
            PreviewMode::Tab => {
                if let Some(tab) = &self.preview_tab {
                    // On the preview tab: close it and go back to the file.
                    let source = tab.source;
                    self.close_window();
                    if let Some(index) = self.index_of_id(source) {
                        let _ = self.select_window(index);
                    }
                    self.status = "Markdown preview tab closed".into();
                } else if !self.is_markdown() {
                    self.status = "The preview is for Markdown (.md) files".into();
                } else if let Some(index) = self.find_preview_tab(self.buffer_id) {
                    let _ = self.select_window(index);
                    self.status = "Markdown preview tab (Alt+F6 closes it)".into();
                } else {
                    let tab = PreviewTab {
                        source: self.buffer_id,
                        title: self.window_names()[self.active].trim_end_matches('*').to_string(),
                        dir: self.path.as_deref().and_then(Path::parent).map(Path::to_path_buf),
                    };
                    self.open_window(Buffer { preview: Some(tab), ..Buffer::default() });
                    self.status = "Markdown preview tab (Alt+F6 closes it)".into();
                }
                focus
            }
            PreviewMode::Window => {
                if let Some(id) = self.preview_window.take() {
                    self.status = "Markdown preview window closed".into();
                    return window::close(id);
                }
                if !self.is_markdown() {
                    self.status = "The preview is for Markdown (.md) files".into();
                    return focus;
                }
                let settings = window::Settings { size: Size::new(760.0, 840.0), ..window::Settings::default() };
                let (id, open) = window::open(settings);
                self.preview_window = Some(id);
                self.status = "Markdown preview window opened (Alt+F6 closes it)".into();
                Task::batch([open.map(|_| Message::WindowOpened), focus])
            }
        }
    }

    /// Reads the source window's text again when the preview tab becomes the active window.
    fn refresh_preview_tab(&mut self) {
        let Some(tab) = &self.preview_tab else { return };
        let text = match self.background.iter().find(|b| b.id == tab.source) {
            Some(source) => source.content.text(),
            None => "The file of this preview is closed.".into(),
        };
        self.preview_dir = tab.dir.clone();
        if text != self.preview_source {
            self.preview_content = markdown::Content::parse(&text);
            self.preview_source = text;
        }
    }

    /// Re-reads the text when it changed and scrolls the preview to where the
    /// cursor is, so it follows what is being edited.
    fn sync_preview(&mut self) -> Task<Message> {
        use iced::widget::scrollable::RelativeOffset;
        if !self.preview_live() {
            return Task::none();
        }
        let dir = self.path.as_deref().and_then(Path::parent);
        if self.preview_dir.as_deref() != dir {
            self.preview_dir = dir.map(Path::to_path_buf);
        }
        let text = self.content.text();
        if text != self.preview_source {
            self.preview_content = markdown::Content::parse(&text);
            self.preview_source = text;
        }
        let line = self.content.cursor().position.line;
        if line == self.preview_line {
            return Task::none();
        }
        self.preview_line = line;
        let last = self.content.line_count().saturating_sub(1).max(1);
        let y = (line as f32 / last as f32).clamp(0.0, 1.0);
        iced::widget::operation::snap_to(PREVIEW_SCROLL, RelativeOffset { x: 0.0, y })
    }

    fn open_link(&mut self, url: &str) {
        let dir = self.preview_dir.as_deref().or_else(|| self.path.as_deref().and_then(Path::parent));
        match classify_link(url, dir) {
            LinkTarget::Web(url) => {
                self.status = match platform::open_url(&url) {
                    Ok(()) => format!("Opened in the browser: {url}"),
                    Err(e) => format!("Cannot open the browser: {e}"),
                };
            }
            LinkTarget::File(path) => {
                self.open_path(&path);
            }
            LinkTarget::Heading => self.status = "Links to a heading are not followed yet".into(),
            LinkTarget::Unknown => self.status = format!("Cannot open the link: {url}"),
        }
    }

    /// The rendered Markdown, on a light page, in its own scrolling pane.
    fn preview_pane(&self) -> Element<'_, Message> {
        let mut style = markdown::Style::from(&Theme::Dark);
        style.link_color = turbo_blue();
        let settings = markdown::Settings::with_text_size(self.font_size() as u32, style);
        let page = markdown::view(self.preview_content.items(), settings).map(Message::PreviewLink);
        container(scrollable(container(page).padding(12)).id(PREVIEW_SCROLL).width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(turbo_gray())),
                text_color: Some(Color::BLACK),
                border: iced::Border::default().color(Color::BLACK).width(1),
                ..container::Style::default()
            })
            .into()
    }

    /// Vertical tabs: the open files as a list on the left of the editor.
    fn window_list(&self) -> Element<'_, Message> {
        let rows = self.window_names().into_iter().enumerate().map(|(index, name)| {
            let active = index == self.active;
            button(
                text(truncate(&format!("{} {name}", index + 1), 22))
                    .color(if active { Color::BLACK } else { turbo_dark_gray() }),
            )
            .width(Length::Fill)
            .padding([2, 6])
            .on_press(Message::SelectWindow(index))
            .style(move |_, status| button::Style {
                background: Some(Background::Color(
                    if active || matches!(status, button::Status::Hovered) { turbo_cyan() } else { turbo_gray() },
                )),
                ..button::Style::default()
            })
            .into()
        });
        container(scrollable(column(rows)))
            .width(236)
            .height(Length::Fill)
            .style(|_| container::Style::default().background(turbo_gray()))
            .into()
    }

    /// Legacy windows: Turbo Pascal's window frame line with the file name and
    /// the window number.
    fn legacy_title(&self) -> Element<'_, Message> {
        let name = self.window_names()[self.active].clone();
        let (left, right) = legacy_title_text(&name, self.active + 1, self.window_count());
        container(
            row![
                text(left).color(Color::WHITE),
                container(text("\u{2550}".repeat(300)).color(Color::WHITE).wrapping(text::Wrapping::None))
                    .width(Length::Fill)
                    .clip(true),
                text(right).color(Color::WHITE),
            ]
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .style(|_| container::Style::default().background(turbo_blue()))
        .into()
    }

    /// Watches, locals and the call stack, shown while debugging.
    fn debug_panel(&self) -> Element<'_, Message> {
        let header = |title: &str| text(format!(" {title}")).color(Color::WHITE);
        let line = |left: String, right: String| {
            row![text(left).color(Color::BLACK).width(Length::FillPortion(2)), text(right).color(turbo_blue()).width(Length::FillPortion(3))]
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
            None => (column![text(" (not running)").color(turbo_dark_gray())], column![]),
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
        .style(|_| container::Style::default().background(turbo_cyan()))
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
        if self.language.is_some() {
            return self.language_key(key);
        }
        if self.settings.is_some() {
            return self.settings_key(key);
        }
        if self.toolchains.is_some() {
            return self.toolchain_key(key);
        }
        if self.project.is_some() {
            return self.project_key(key);
        }
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
        if self.search.is_some() && key == NavKey::BackTab {
            return iced::widget::operation::focus_previous();
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
            NavKey::Left | NavKey::BackTab => self.open_menu(step(-1)),
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
        // Tab walks the rows, then the bottom controls, then round again.
        match (key, dialog.button) {
            (NavKey::Tab, None) if dialog.selected + 1 < count => dialog.selected += 1,
            (NavKey::Tab, None) => dialog.button = Some(0),
            (NavKey::Tab, Some(b)) if b < 3 => dialog.button = Some(b + 1),
            (NavKey::Tab, Some(_)) => {
                dialog.button = None;
                dialog.selected = 0;
            }
            (NavKey::BackTab, None) if dialog.selected > 0 => dialog.selected -= 1,
            (NavKey::BackTab, None) => dialog.button = Some(3),
            (NavKey::BackTab, Some(0)) => {
                dialog.button = None;
                dialog.selected = count.saturating_sub(1);
            }
            (NavKey::BackTab, Some(b)) => dialog.button = Some(b - 1),
            (NavKey::Left, Some(b)) => dialog.button = Some(b.saturating_sub(1)),
            (NavKey::Right, Some(b)) => dialog.button = Some((b + 1).min(3)),
            (NavKey::Up | NavKey::Down, Some(_)) => dialog.button = None,
            (NavKey::Enter | NavKey::Char(' '), Some(b)) => {
                return match b {
                    0 => {
                        let on = !self.check_at_startup;
                        self.update(Message::ToggleStartupCheck(on))
                    }
                    1 => self.update(Message::OpenUrl(platform::INSTALL_GUIDE_URL)),
                    2 if !busy => self.update(Message::ShowDeps),
                    2 => Task::none(),
                    _ => self.update(Message::CloseDeps),
                };
            }
            (NavKey::Up, None) if count > 0 => dialog.selected = (dialog.selected + count - 1) % count,
            (NavKey::Down, None) if count > 0 => dialog.selected = (dialog.selected + 1) % count,
            (NavKey::PageUp, None) => dialog.selected = dialog.selected.saturating_sub(4),
            (NavKey::PageDown, None) => dialog.selected = (dialog.selected + 4).min(count.saturating_sub(1)),
            (NavKey::Home, None) => dialog.selected = 0,
            (NavKey::End, None) => dialog.selected = count.saturating_sub(1),
            _ => {}
        }
        let selected = dialog.selected;
        let dep = dialog.list.get(selected).filter(|d| d.missing());
        let can_install = dep.is_some_and(|d| d.auto.is_some()) && !busy;
        let install_text = dep.map(|d| d.install.clone());
        // Keep the selected row in view: the rows are about the same height.
        let scroll = if matches!(key, NavKey::Up | NavKey::Down | NavKey::Tab | NavKey::BackTab | NavKey::PageUp | NavKey::PageDown | NavKey::Home | NavKey::End) && count > 1 {
            let y = selected as f32 / (count - 1) as f32;
            iced::widget::operation::snap_to(DEPS_SCROLL, scrollable::RelativeOffset { x: 0.0, y })
        } else {
            Task::none()
        };
        let task = match key {
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
        };
        Task::batch([scroll, task])
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
            SearchMode::Toolchain => {
                rust_ide::env::rust_toolchain(&self.project_dir()).map(|(name, _)| name).unwrap_or_default()
            }
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
        if matches!(bar.mode, SearchMode::GoTo | SearchMode::Watch | SearchMode::Toolchain) || bar.query.is_empty() {
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
                text("an expression, evaluated at each stop").color(turbo_dark_gray()),
                Space::new().width(Length::Fill),
                close,
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into(),
            SearchMode::Toolchain => row![
                label(" Rust toolchain:"),
                input(&bar.query, Message::SearchQuery, Message::SetToolchain, 220.0).id(SEARCH_INPUT),
                small_button("Set", Some(Message::SetToolchain)),
                text("stable, nightly, 1.85.0 ... (empty = default); saved in the project environment")
                    .color(turbo_dark_gray()),
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
                text(format!("(1-{})", self.content.line_count())).color(turbo_dark_gray()),
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
                ..container::Style::default().background(turbo_gray())
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
                vim::Effect::Quit | vim::Effect::ForceQuit => tasks.push(self.exit_ide()),
                vim::Effect::SaveQuit => {
                    if self.path.is_none() {
                        self.status = "No file name (use F2 to save first)".into();
                    } else {
                        tasks.push(self.save(AfterSave::Nothing));
                        if !self.modified {
                            tasks.push(self.exit_ide());
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
            Discard::Close => "Ctrl+F3",
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
            return self.save_as(after);
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

    fn view(&self, window: window::Id) -> Element<'_, Message> {
        if Some(window) == self.preview_window {
            return self.preview_pane();
        }
        self.main_view()
    }

    fn main_view(&self) -> Element<'_, Message> {
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
        .style(|_| container::Style::default().background(turbo_gray()));

        let popup_open = self.completion.is_some();
        // While a menu, dialog or the help screen is open, keys drive it, not the text.
        let modal = self.menu.is_some()
            || self.help
            || self.deps.is_some()
            || self.language.is_some()
            || self.settings.is_some()
            || self.toolchains.is_some()
            || self.project.is_some();
        let vim_mode = self.vim.as_ref().map(|v| (v.mode, v.command.is_some()));
        let editor = text_editor(&self.content)
            .id(EDITOR)
            .size(self.font_size() as f32)
            .on_action(Message::Edit)
            .height(Length::Fill)
            .highlight_with::<highlight::Highlighter>(
                highlight::Settings {
                    lang: self.lang(),
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
                background: Background::Color(turbo_blue()),
                value: turbo_yellow(),
                ..text_editor::default(theme, status)
            });

        let gutter = self.line_numbers.then_some(cursor::Gutter {
            background: if platform::mono() { Color::from_rgb(0.12, 0.12, 0.12) } else { Color::from_rgb(0.0, 0.0, 0.5) },
            number: turbo_cyan(),
            current: turbo_yellow(),
        });
        let editor = cursor::BlockCursor::new(editor, &self.content, turbo_yellow(), turbo_blue(), self.font_size() as f32)
            .line_numbers(gutter);

        let panel = self.panel_items();
        let mut layout = column![menu_bar];
        match self.window_style {
            WindowStyle::Legacy => layout = layout.push(self.legacy_title()),
            WindowStyle::Horizontal if self.window_count() > 1 => layout = layout.push(self.window_tabs()),
            _ => {}
        }
        if let Some(bar) = &self.search {
            layout = layout.push(self.search_bar(bar));
        }
        let editor: Element<'_, Message> = if self.preview_tab.is_some() {
            self.preview_pane()
        } else if self.preview_visible() {
            row![editor, self.preview_pane()].spacing(2).height(Length::Fill).into()
        } else {
            editor.into()
        };
        layout = if self.window_style == WindowStyle::Vertical && self.window_count() > 1 {
            layout.push(row![self.window_list(), editor].height(Length::Fill))
        } else {
            layout.push(editor)
        };
        if self.debug.is_some() || !self.watches.is_empty() {
            layout = layout.push(self.debug_panel());
        } else if !panel.is_empty() {
            layout = layout.push(self.messages_panel(panel));
        }
        layout = layout.push(self.status_bar());

        let mut layers = stack![layout];
        if let Some(popup) = self.completion_popup() {
            let bottom = status_bar_height()
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
        if let Some(dialog) = &self.language {
            layers = layers.push(language_dialog(dialog));
        }
        if let Some(dialog) = &self.settings {
            layers = layers.push(self.settings_dialog(dialog));
        }
        if let Some(dialog) = &self.toolchains {
            layers = layers.push(toolchain_dialog(dialog));
        }
        if let Some(dialog) = &self.project {
            layers = layers.push(project_dialog(dialog));
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
        .style(|_| container::Style::default().background(turbo_gray()));

        let screen = scrollable(
            text(&self.output.screen).color(SCREEN_TEXT).font(Font::MONOSPACE).size(self.font_size() as f32).width(Length::Fill),
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
                container::Style::default().background(turbo_gray())
            }),
        ])
        .style(|_| container::Style {
            border: iced::Border::default().color(turbo_gray()).width(2),
            ..container::Style::default().background(Color::BLACK)
        });

        opaque(
            container(window)
                .padding(Padding::default().top(MENU_BAR_HEIGHT + 8.0).bottom(status_bar_height() + 8.0).left(24.0).right(24.0))
                .width(Length::Fill)
                .height(Length::Fill),
        )
    }

    fn deps_dialog<'a>(&'a self, dialog: &'a DepsDialog) -> Element<'a, Message> {
        let busy = dialog.checking || dialog.installing.is_some();
        let rows = dialog.list.iter().enumerate().map(|(index, dep)| {
            let (mark, color) = match (&dep.found, dep.need) {
                (Some(_), _) => ("[ OK ]", if platform::mono() { Color::BLACK } else { turbo_green() }),
                (None, deps::Need::Optional) => ("[ -- ]", turbo_dark_gray()),
                (None, _) => ("[MISS]", turbo_red()),
            };
            let need = match dep.need {
                deps::Need::Required => "required",
                deps::Need::Recommended => "recommended",
                deps::Need::Optional => "optional",
            };
            let detail: Element<'a, Message> = match &dep.found {
                Some(found) => text(truncate(found, 52)).color(turbo_dark_gray()).into(),
                None => row![
                    text(&dep.install).color(turbo_blue()).width(Length::Fill),
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
                        text(format!("{} ({need})", dep.purpose)).color(turbo_dark_gray()).size(12),
                    ]
                    .width(300),
                    detail,
                ]
                .spacing(8),
            )
            .padding([2, 4])
            .style(move |_| {
                if selected {
                    container::Style::default().background(turbo_cyan())
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
        let focus = dialog.button;
        let buttons = row![
            container(checkbox(self.check_at_startup)
                .label("Check at startup")
                .on_toggle(Message::ToggleStartupCheck)
                .style(|theme, status| iced::widget::checkbox::Style {
                    text_color: Some(Color::BLACK),
                    ..iced::widget::checkbox::primary(theme, status)
                }))
            .padding(2)
            .style(move |_| {
                if focus == Some(0) {
                    container::Style::default().background(turbo_cyan())
                } else {
                    container::Style::default()
                }
            }),
            Space::new().width(Length::Fill),
            small_button_focus("Install guide on GitHub", Some(Message::OpenUrl(platform::INSTALL_GUIDE_URL)), focus == Some(1)),
            small_button_focus("Re-check", (!busy).then_some(Message::ShowDeps), focus == Some(2)),
            small_button_focus("Close", Some(Message::CloseDeps), focus == Some(3)),
        ]
        .spacing(8);

        let body = column![
            container(header).width(Length::Fill).padding(2).style(|_| {
                container::Style::default().background(turbo_blue())
            }),
            scrollable(column(rows).spacing(6).padding(Padding::default().top(10).bottom(10).left(10).right(24)))
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new().width(10).scroller_width(10).margin(4),
                ))
                .id(DEPS_SCROLL)
                .height(Length::Fill),
            container(text(message).color(turbo_red())).padding([0, 10]),
            container(
                text(" Up/Down/Tab select   I install   C copy   G guide   R re-check   S startup check   Esc close")
                    .color(turbo_dark_gray()),
            )
            .padding([4, 10]),
            container(buttons).padding(10),
        ];

        // The list scrolls when the window is too short for every row.
        opaque(
            container(
                container(body).width(900).max_height(540).style(|_| container::Style {
                    border: iced::Border::default().color(Color::BLACK).width(1),
                    ..container::Style::default().background(turbo_gray())
                }),
            )
            .padding(Padding::default().top(MENU_BAR_HEIGHT + 8.0).bottom(status_bar_height() + 8.0))
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
                    text(format!("{:<8}", item.kind)).color(turbo_dark_gray()),
                    text(&item.label).color(Color::BLACK),
                    Space::new().width(Length::Fill),
                    text(detail).color(turbo_dark_gray()),
                ])
                .width(Length::Fill)
                .padding([1, 6])
                .on_press(Message::CompletionPick(index))
                .style(move |_, status| button::Style {
                    background: (selected || matches!(status, button::Status::Hovered))
                        .then(|| Background::Color(turbo_green())),
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
            let color = if d.level == "error" { turbo_red() } else { Color::BLACK };
            let chosen = self.message_index == Some(index);
            button(text(format!("{} {location}{}", d.level.to_uppercase(), d.message)).color(color))
                .width(Length::Fill)
                .padding([1, 8])
                .on_press(Message::JumpTo(index))
                .style(move |_, status| button::Style {
                    background: (chosen || matches!(status, button::Status::Hovered))
                        .then(|| Background::Color(turbo_green())),
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
        .style(|_| container::Style::default().background(turbo_cyan()))
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

    fn set_status_lines(&mut self, two: bool) {
        self.two_line_status = two;
        TWO_LINE_STATUS.store(two, std::sync::atomic::Ordering::Relaxed);
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
        if self.two_line_status {
            // Line 1: what is happening. Line 2: the keys, in smaller text.
            let size = 12;
            let top = row![
                container(text(format!(" {message}")).size(size).color(turbo_red()).wrapping(text::Wrapping::None))
                    .width(Length::Fill)
                    .clip(true),
                text(self.toolchain_label()).size(size).color(turbo_dark_gray()),
                text(format!("  {}  ", truncate(&self.lsp_status, 30))).size(size).color(turbo_dark_gray()),
                text(self.vim.as_ref().map(|v| format!("{}  ", v.status())).unwrap_or_default())
                    .size(size)
                    .color(turbo_dark_gray()),
                text(format!("{}:{} ", cursor.line + 1, self.cursor_char_column() + 1)).size(size).color(Color::BLACK),
            ];
            let keys = " F1 Help  F2 Save  F3 Open  F4 Go to cursor  F5 Run  F7 Trace  F8 Step  F9 Compile  Alt+F9 Check  Ctrl+F8 Breakpoint  F10 Menu";
            return container(column![top, text(keys).size(size).color(Color::BLACK).wrapping(text::Wrapping::None)])
                .padding([2, 4])
                .height(status_bar_height())
                .width(Length::Fill)
                .clip(true)
                .style(|_| container::Style::default().background(turbo_gray()))
                .into();
        }
        container(row![
            text(keys).color(Color::BLACK),
            // The message takes the remaining space and is clipped, so the
            // server status and cursor position always stay visible.
            container(text(format!("   {message}")).color(turbo_red()).wrapping(text::Wrapping::None))
                .width(Length::Fill)
                .clip(true),
            text(self.toolchain_label()).color(turbo_dark_gray()),
            text(format!("  {}  ", truncate(&self.lsp_status, 30))).color(turbo_dark_gray()),
            text(self.vim.as_ref().map(|v| format!("{}  ", v.status())).unwrap_or_default())
                .color(turbo_blue()),
            text(format!("{}:{} ", cursor.line + 1, self.cursor_char_column() + 1)).color(Color::BLACK),
        ])
        .padding([2, 4])
        .height(status_bar_height())
        .width(Length::Fill)
        .style(|_| container::Style::default().background(turbo_gray()))
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
        Key::Named(key::Named::F3) if modifiers.control() => Some(Message::CloseWindow),
        Key::Named(key::Named::F3) => Some(Message::Open),
        Key::Named(key::Named::F6) if modifiers.alt() => Some(Message::TogglePreview),
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
            Some('p') if modifiers.alt() && !modifiers.control() => Some(Message::TogglePreview),
            Some(c @ '1'..='9') if modifiers.alt() => Some(Message::SelectWindow(c as usize - '1' as usize)),
            Some(c) if modifiers.alt() && !modifiers.control() => Menu::from_hotkey(c).map(Message::OpenMenu),
            Some('=' | '+') if modifiers.command() => Some(Message::FontLarger),
            Some('-') if modifiers.command() => Some(Message::FontSmaller),
            Some('0') if modifiers.command() => Some(Message::FontDefault),
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
            ..container::Style::default().background(turbo_cyan())
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

fn save_as_dialog(after: AfterSave, name: String, ext: String, dir: Option<PathBuf>) -> Task<Message> {
    let mut dialog = rfd::AsyncFileDialog::new().set_title("Save As").set_file_name(name);
    if let Some(dir) = dir {
        dialog = dialog.set_directory(dir);
    }
    // The file's own type comes first in the list of types.
    let mut kinds: Vec<&filetype::FileType> = filetype::FILE_TYPES.iter().collect();
    kinds.sort_by_key(|kind| kind.ext != ext);
    for kind in kinds {
        dialog = dialog.add_filter(kind.label, &[kind.ext]);
    }
    Task::perform(
        dialog.add_filter("All files", &["*"]).save_file(),
        move |file| Message::SaveAsPicked(file.map(|f| f.path().to_path_buf()), after),
    )
}

/// The "choose a language" list.
/// What a click on a link in the preview should do.
#[derive(Debug, PartialEq)]
enum LinkTarget {
    /// A web address (or mail address): the system's browser opens it.
    Web(String),
    /// A file next to the document: it opens in a window.
    File(PathBuf),
    /// `#heading` inside the document.
    Heading,
    Unknown,
}

/// Only http, https and mailto go to the browser; anything else that is not a
/// file next to the document is refused.
fn classify_link(url: &str, doc_dir: Option<&Path>) -> LinkTarget {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:") {
        return LinkTarget::Web(url.to_string());
    }
    if url.starts_with('#') {
        return LinkTarget::Heading;
    }
    let file = url.split('#').next().unwrap_or("").replace("%20", " ");
    match doc_dir {
        Some(dir) if !file.is_empty() && !lower.contains("://") => {
            let candidate = dir.join(file);
            if candidate.is_file() { LinkTarget::File(candidate) } else { LinkTarget::Unknown }
        }
        _ => LinkTarget::Unknown,
    }
}

/// The two ends of the legacy window title line: the file name on the left
/// and the window number on the right, joined by the frame line.
fn legacy_title_text(name: &str, number: usize, count: usize) -> (String, String) {
    (format!("\u{2550}\u{2550}[\u{25A0}]\u{2550}\u{2550} {name} "), format!(" {number}/{count} \u{2550}\u{2550}"))
}

fn project_dialog(dialog: &ProjectDialog) -> Element<'static, Message> {
    let choice = |row: usize, label: &str, hotkey: char, value: String| {
        let chosen = dialog.row == row;
        button(row![
            hotkey_label(label, hotkey),
            Space::new().width(Length::Fill),
            text(format!("< {value} >")).color(Color::BLACK),
        ])
        .width(Length::Fill)
        .padding([3, 10])
        .on_press(Message::ProjectAct(row, 1))
        .style(move |_, status| button::Style {
            background: Some(Background::Color(
                if chosen || matches!(status, button::Status::Hovered) { turbo_green() } else { turbo_gray() },
            )),
            ..button::Style::default()
        })
    };
    let field = |row: usize, label: &'static str, value: &str, on_input: fn(String) -> Message, id: &'static str| {
        let chosen = dialog.row == row;
        container(
            iced::widget::row![
                text(label).color(Color::BLACK).width(80),
                text_input("", value)
                    .id(id)
                    .on_input(on_input)
                    .on_submit(Message::ProjectCreate)
                    .font(Font::MONOSPACE)
                    .padding([2, 6]),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([3, 10])
        .width(Length::Fill)
        .style(move |_| container::Style::default().background(if chosen { turbo_green() } else { turbo_gray() }))
    };
    let action = |row: usize, label: &str, hotkey: char| {
        let chosen = dialog.row == row;
        button(hotkey_label(label, hotkey))
            .width(Length::Fill)
            .padding([3, 10])
            .on_press(Message::ProjectAct(row, 0))
            .style(move |_, status| button::Style {
                background: Some(Background::Color(
                    if chosen || matches!(status, button::Status::Hovered) { turbo_green() } else { turbo_gray() },
                )),
                ..button::Style::default()
            })
    };
    let problem = match templates::check_name(&dialog.name) {
        _ if !dialog.error.is_empty() => dialog.error.clone(),
        Err(why) if !dialog.name.is_empty() => why,
        _ => String::new(),
    };
    let target = std::path::Path::new(dialog.folder.trim()).join(dialog.name.trim());
    let body = column![
        container(text(" New project ").color(Color::WHITE))
            .width(Length::Fill)
            .padding(2)
            .style(|_| container::Style::default().background(turbo_blue())),
        column![
            choice(PROJECT_LANG, "Language", 'l', dialog.lang.label().to_string()),
            choice(PROJECT_SIZE, "Size", 's', dialog.size.label().to_string()),
            field(PROJECT_NAME, "Name", &dialog.name, Message::ProjectName, PROJECT_NAME_INPUT),
            field(PROJECT_FOLDER, "Folder", &dialog.folder, Message::ProjectFolder, PROJECT_FOLDER_INPUT),
            action(PROJECT_CREATE, "Create", 'c'),
            action(PROJECT_CREATE + 1, "Cancel", '\0'),
        ]
        .padding(8),
        container(text(dialog.size.describe(dialog.lang)).color(turbo_dark_gray()).size(13)).padding([0, 10]),
        container(text(format!("Creates {}", target.display())).color(turbo_dark_gray()).size(13)).padding([0, 10]),
        container(text(problem).color(turbo_red()).size(13)).padding([2, 10]),
        container(text(" Tab/Up/Down field  Left/Right or letters change  Enter Create  Esc cancels").color(Color::BLACK).size(13))
            .padding(10),
    ];
    opaque(
        container(container(body).width(620).style(|_| container::Style {
            border: iced::Border::default().color(Color::BLACK).width(1),
            ..container::Style::default().background(turbo_gray())
        }))
        .center(Length::Fill),
    )
}

fn language_dialog(dialog: &LanguageDialog) -> Element<'static, Message> {
    let rows = filetype::FILE_TYPES.iter().enumerate().map(|(index, kind)| {
        let chosen = index == dialog.selected;
        button(row![
            hotkey_label(kind.label, kind.hotkey),
            Space::new().width(Length::Fill),
            text(format!(".{}", kind.ext)).color(Color::BLACK),
        ])
        .width(Length::Fill)
        .padding([3, 10])
        .on_press(Message::NewFileType(index))
        .style(move |_, status| button::Style {
            background: Some(Background::Color(
                if chosen || matches!(status, button::Status::Hovered) { turbo_green() } else { turbo_gray() },
            )),
            ..button::Style::default()
        })
        .into()
    });
    let (title, escape) = if dialog.startup {
        (" Choose a language ", "Esc: Rust")
    } else {
        (" New file: choose a language ", "Esc: cancel")
    };
    let body = column![
        container(text(title).color(Color::WHITE))
            .width(Length::Fill)
            .padding(2)
            .style(|_| container::Style::default().background(turbo_blue())),
        column(rows).padding(8),
        container(text(filetype::NOTE).color(turbo_dark_gray()).size(12)).padding([0, 10]),
        container(text(format!(" Up/Down  Enter OK  R P T M or 1-4  {escape}")).color(Color::BLACK)).padding(10),
    ];
    opaque(
        container(container(body).width(520).style(|_| container::Style {
            border: iced::Border::default().color(Color::BLACK).width(1),
            ..container::Style::default().background(turbo_gray())
        }))
        .center(Length::Fill),
    )
}

fn dropdown(menu: Menu, toggles: Toggles, selected: usize, windows: &[String], active: usize) -> Element<'static, Message> {
    let entries = menu_items(menu, toggles, windows, active).into_iter().enumerate().map(|(index, item)| {
        let enabled = item.message.is_some();
        let label: Element<'static, Message> = if enabled {
            hotkey_label(&item.label, item.hotkey)
        } else {
            text(item.label).color(turbo_dark_gray()).into()
        };
        let chosen = index == selected && enabled;
        button(row![label, Space::new().width(Length::Fill), text(item.keys).color(Color::BLACK)])
            .width(290)
            .padding([3, 10])
            .on_press_maybe(enabled.then_some(Message::MenuPick(index)))
            .style(move |_, status| button::Style {
                background: Some(Background::Color(
                    if chosen || (enabled && matches!(status, button::Status::Hovered)) {
                        turbo_green()
                    } else {
                        turbo_gray()
                    },
                )),
                ..button::Style::default()
            })
            .into()
    });

    container(container(column(entries)).style(|_| container::Style {
        border: iced::Border::default().color(Color::BLACK).width(1),
        ..container::Style::default().background(turbo_gray())
    }))
    .padding(Padding::default().top(MENU_BAR_HEIGHT).left(menu.index() as f32 * MENU_TITLE_WIDTH))
    .into()
}

/// A label with its hotkey letter in red, Turbo Pascal style.
/// The toolchain picker: rows for the default and every installed toolchain,
/// each with a hint, and "Other name..." at the end.
fn toolchain_dialog(dialog: &ToolchainDialog) -> Element<'static, Message> {
    let row_button = |index: usize, name: String, note: String, mark: &'static str, usable: bool| {
        let chosen = index == dialog.selected;
        let note_color = if usable { turbo_dark_gray() } else { turbo_red() };
        button(column![
            row![text(name).color(Color::BLACK), Space::new().width(Length::Fill), text(mark).color(Color::BLACK)],
            text(note).color(note_color).size(13),
        ])
        .width(Length::Fill)
        .padding([3, 10])
        .on_press(Message::ToolchainPick(index))
        .style(move |_, status| button::Style {
            background: Some(Background::Color(
                if chosen || matches!(status, button::Status::Hovered) { turbo_green() } else { turbo_gray() },
            )),
            ..button::Style::default()
        })
    };
    let mut rows: Vec<Element<'static, Message>> = dialog
        .choices
        .iter()
        .enumerate()
        .map(|(index, c)| {
            let name = if c.name.is_empty() { "Default".to_string() } else { c.name.clone() };
            row_button(index, name, c.note.clone(), if c.current { "(*)" } else { "( )" }, c.usable).into()
        })
        .collect();
    rows.push(row_button(dialog.choices.len(), "Other name...".into(), "type a toolchain name yourself".into(), "", true).into());
    let body = column![
        container(text(" Rust toolchain of this project ").color(Color::WHITE))
            .width(Length::Fill)
            .padding(2)
            .style(|_| container::Style::default().background(turbo_blue())),
        scrollable(column(rows).padding(8)).height(Length::Shrink),
        container(
            text(" Up/Down  Enter picks  O other name  Esc closes. Install more with: rustup toolchain install <name>")
                .color(Color::BLACK)
                .size(13)
        )
        .padding(10),
    ];
    opaque(
        container(container(body).width(640).style(|_| container::Style {
            border: iced::Border::default().color(Color::BLACK).width(1),
            ..container::Style::default().background(turbo_gray())
        }))
        .center(Length::Fill),
    )
}

fn hotkey_label(label: &str, hotkey: char) -> Element<'static, Message> {
    let at = label.char_indices().find(|(_, c)| c.eq_ignore_ascii_case(&hotkey)).map(|(i, _)| i);
    match at {
        Some(i) => {
            let end = i + label[i..].chars().next().map_or(0, char::len_utf8);
            row![
                text(label[..i].to_string()).color(Color::BLACK),
                text(label[i..end].to_string()).color(turbo_red()),
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
            lines.push(text(format!(" {g}")).color(turbo_yellow()).into());
        }
        lines.push(
            row![text(format!("   {keys}")).color(Color::WHITE).width(330), text(action).color(SCREEN_TEXT)].into(),
        );
    }
    let title = container(
        text(" Keyboard shortcuts    Up/Down/PgUp/PgDn scroll    Esc or F1 closes ").color(Color::BLACK),
    )
    .width(Length::Fill)
    .style(|_| container::Style::default().background(turbo_gray()));
    let list = scrollable(column(lines).spacing(2).padding([6, 8])).id(HELP_SCROLL).height(Length::Fill);
    let window = container(column![title, list]).style(|_| container::Style {
        border: iced::Border::default().color(turbo_gray()).width(2),
        ..container::Style::default().background(turbo_cyan())
    });
    opaque(
        container(window)
            .padding(Padding::default().top(MENU_BAR_HEIGHT + 8.0).bottom(status_bar_height() + 8.0).left(60.0).right(60.0))
            .width(Length::Fill)
            .height(Length::Fill),
    )
}

/// A Turbo-style dialog button; disabled when `message` is None.
fn small_button(label: &str, message: Option<Message>) -> Element<'_, Message> {
    small_button_focus(label, message, false)
}

/// A small button; `focused` marks the one Tab has reached (cyan with a black frame).
fn small_button_focus(label: &str, message: Option<Message>, focused: bool) -> Element<'_, Message> {
    let enabled = message.is_some();
    button(text(format!(" {label} ")).color(if enabled { Color::BLACK } else { turbo_dark_gray() }))
        .padding([1, 6])
        .on_press_maybe(message)
        .style(move |_, status| button::Style {
            background: Some(Background::Color(
                if focused || (enabled && matches!(status, button::Status::Hovered)) { turbo_cyan() } else { turbo_green() },
            )),
            text_color: Color::BLACK,
            border: if focused { iced::Border::default().color(Color::BLACK).width(2) } else { iced::Border::default() },
            ..button::Style::default()
        })
        .into()
}

fn menu_title_style(open: bool, status: button::Status) -> button::Style {
    let highlighted = open || matches!(status, button::Status::Hovered);
    button::Style {
        background: highlighted.then(|| Background::Color(turbo_green())),
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

    fn vim_keys(ide: &mut Ide, keys: &str) {
        for ch in keys.chars() {
            let _ = ide.update(Message::Vim(vim::Key::Char(ch)));
        }
    }

    #[test]
    fn vim_dot_text_objects_marks_and_visual_line_through_the_ide() {
        let mut ide = ide("f(a, b) x\nline two\nline three");
        ide.vim = Some(vim::Vim::default());
        vim_keys(&mut ide, "fadi(");
        assert_eq!(ide.content.text(), "f() x\nline two\nline three");
        assert!(ide.modified);
        vim_keys(&mut ide, "jma");
        vim_keys(&mut ide, "ggfxx");
        vim_keys(&mut ide, "j0.");
        assert_eq!(ide.content.text(), "f() \nine two\nline three", "dot repeats x");
        vim_keys(&mut ide, "G`a");
        assert_eq!(ide.content.cursor().position.line, 1, "jump to the mark");
        vim_keys(&mut ide, "ggVjd");
        assert_eq!(ide.content.text(), "line three");
        assert_eq!(ide.vim.as_ref().map(|v| v.status()).as_deref(), Some("NORMAL"));
        vim_keys(&mut ide, "u");
        assert_eq!(ide.content.text(), "f() \nine two\nline three", "one undo step for the line delete");
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
        assert_eq!(ide.menu_item, 5, "Up from the first item wraps to Exit");

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
            button: None,
        });
        send(&mut ide, [Message::Nav(NavKey::Down)]);
        assert_eq!(ide.deps.as_ref().map(|d| d.selected), Some(1));
        send(&mut ide, [Message::Nav(NavKey::Char('c'))]);
        assert_eq!(ide.status, "Command copied to the clipboard");
        send(&mut ide, [Message::Nav(NavKey::Down)]);
        assert_eq!(ide.deps.as_ref().map(|d| d.selected), Some(0), "wraps around");
        send(&mut ide, [Message::Nav(NavKey::Tab)]);
        assert_eq!(ide.deps.as_ref().map(|d| d.selected), Some(1), "Tab moves down");
        send(&mut ide, [Message::Nav(NavKey::BackTab)]);
        assert_eq!(ide.deps.as_ref().map(|d| d.selected), Some(0), "Shift+Tab moves up");
        send(&mut ide, [Message::Nav(NavKey::BackTab)]);
        assert_eq!(ide.deps.as_ref().map(|d| d.button), Some(Some(3)), "Shift+Tab from the top reaches Close");
        send(&mut ide, [Message::Nav(NavKey::BackTab), Message::Nav(NavKey::BackTab), Message::Nav(NavKey::BackTab)]);
        assert_eq!(ide.deps.as_ref().map(|d| d.button), Some(Some(0)));
        send(&mut ide, [Message::Nav(NavKey::BackTab)]);
        assert_eq!(ide.deps.as_ref().map(|d| (d.button, d.selected)), Some((None, 1)), "back to the last row");
        send(&mut ide, [Message::Nav(NavKey::Tab), Message::Nav(NavKey::Tab), Message::Nav(NavKey::Tab), Message::Nav(NavKey::Tab)]);
        assert_eq!(ide.deps.as_ref().map(|d| d.button), Some(Some(3)), "Tab reaches the buttons");
        send(&mut ide, [Message::Nav(NavKey::Enter)]);
        assert!(ide.deps.is_none(), "Enter on Close closes the dialog");
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
        assert_eq!(ide.window_count(), 3, "first Ctrl+F3 only warns");
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
        assert!(matches!(shortcut(&f(key::Named::F3), physical, Modifiers::CTRL), Some(Message::CloseWindow)));
        assert!(!matches!(shortcut(&f(key::Named::F3), physical, alt), Some(Message::CloseWindow)), "Alt+F3 is left to other programs");

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
        let on = Toggles { check_before: true, ..Toggles::default() };
        let items = menu_items(Menu::Options, on, &[], 0);
        let item = items.iter().find(|i| i.label.starts_with("Check before run")).expect("menu item");
        assert!(item.label.ends_with("[x]") && item.hotkey == 'c');
        let items = menu_items(Menu::Options, Toggles::default(), &[], 0);
        assert!(items.iter().any(|i| i.label.starts_with("Check before run") && i.label.ends_with("[ ]")));
    }
}

#[cfg(test)]
mod language_tests {
    use super::*;

    use crate::test_support::isolate_settings;

    fn send(ide: &mut Ide, messages: impl IntoIterator<Item = Message>) {
        for message in messages {
            let _ = ide.update(message);
        }
    }

    fn type_text(ide: &mut Ide, text: &str) {
        for c in text.chars() {
            send(ide, [Message::Edit(Action::Edit(Edit::Insert(c)))]);
        }
    }

    #[test]
    fn startup_chooser_picks_python_with_the_arrow_keys() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        let _ = ide.ask_language(true);
        assert!(ide.language.as_ref().is_some_and(|d| d.startup));
        send(&mut ide, [Message::Nav(NavKey::Home), Message::Nav(NavKey::Down)]);
        assert_eq!(ide.language.as_ref().map(|d| d.selected), Some(1));
        send(&mut ide, [Message::Nav(NavKey::Enter)]);
        assert!(ide.language.is_none());
        assert_eq!(ide.untitled_ext, Some("py"));
        assert_eq!(ide.lang(), Lang::Python);
        assert_eq!(ide.window_count(), 1, "the empty start window is reused");
        assert_eq!(ide.title(window::Id::unique()), "NONAME.PY - Rust IDE");
        assert_eq!(ide.suggested_name(), "NONAME.PY");
    }

    #[test]
    fn letters_digits_and_clicks_pick_a_type() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        let _ = ide.ask_language(false);
        send(&mut ide, [Message::Nav(NavKey::Char('m'))]);
        assert_eq!((ide.untitled_ext, ide.lang()), (Some("md"), Lang::Plain));
        assert_eq!(ide.title(window::Id::unique()), "NONAME.MD - Rust IDE");

        send(&mut ide, [Message::New, Message::Nav(NavKey::Char('4'))]);
        assert_eq!(ide.untitled_ext, Some("txt"));
        assert_eq!(ide.suggested_name(), "NONAME.TXT");

        send(&mut ide, [Message::New, Message::Nav(NavKey::Char('c'))]);
        assert_eq!((ide.untitled_ext, ide.lang()), (Some("cs"), Lang::CSharp));
        assert_eq!(ide.suggested_name(), "NONAME.CS");

        send(&mut ide, [Message::New, Message::NewFileType(0)]);
        assert_eq!((ide.untitled_ext, ide.lang()), (Some("rs"), Lang::Rust));

        send(&mut ide, [Message::New, Message::Nav(NavKey::Char('z'))]);
        assert!(ide.language.is_some(), "an unrelated key leaves the list open");
        assert_eq!(ide.untitled_ext, Some("rs"));
    }

    #[test]
    fn file_new_makes_a_new_window_unless_the_current_one_is_empty() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        type_text(&mut ide, "x");
        send(&mut ide, [Message::New, Message::Nav(NavKey::Char('p'))]);
        assert_eq!(ide.window_count(), 2);
        assert_eq!(ide.window_names(), ["NONAME.RS*", "NONAME.PY"]);
        assert_eq!(ide.active, 1);

        // The first window keeps its own type when switching back and forth.
        send(&mut ide, [Message::SelectWindow(0)]);
        assert_eq!((ide.untitled_ext, ide.lang()), (None, Lang::Plain));
        send(&mut ide, [Message::SelectWindow(1)]);
        assert_eq!((ide.untitled_ext, ide.lang()), (Some("py"), Lang::Python));

        // An empty window is reused by the next choice.
        send(&mut ide, [Message::New, Message::Nav(NavKey::Char('m'))]);
        assert_eq!(ide.window_count(), 2);
        assert_eq!(ide.untitled_ext, Some("md"));
    }

    #[test]
    fn escape_gives_a_rust_window_at_startup_and_cancels_file_new() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        let _ = ide.ask_language(true);
        send(&mut ide, [Message::Escape]);
        assert!(ide.language.is_none());
        assert_eq!(ide.untitled_ext, Some("rs"));

        let mut ide = Ide::default();
        type_text(&mut ide, "x");
        send(&mut ide, [Message::New, Message::Escape]);
        assert!(ide.language.is_none());
        assert_eq!(ide.window_count(), 1, "cancelled: no new window");
    }

    #[test]
    fn the_last_choice_is_preselected_next_time() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        ide.persist_settings = true;
        let _ = ide.ask_language(false);
        send(&mut ide, [Message::Nav(NavKey::Char('m'))]);
        let _ = ide.ask_language(false);
        assert_eq!(ide.language.as_ref().map(|d| d.selected), Some(4), "Markdown, as chosen last time");
        send(&mut ide, [Message::Nav(NavKey::Enter)]);
        assert_eq!(ide.untitled_ext, Some("md"), "Enter alone repeats the last choice");
    }

    #[test]
    fn a_file_name_decides_the_language_and_text_files_open() {
        let _settings = isolate_settings();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("language-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let rs = dir.join("lang.rs");
        let txt = dir.join("notes.txt");
        let md = dir.join("readme.md");
        std::fs::write(&rs, "fn main() {}\n").unwrap();
        std::fs::write(&txt, "plain words\n").unwrap();
        std::fs::write(&md, "# Title\n").unwrap();

        let mut ide = Ide::default();
        ide.untitled_ext = Some("py");
        ide.open_path(&rs);
        assert_eq!(ide.lang(), Lang::Rust, "the file name beats the chosen type");
        assert_eq!(ide.untitled_ext, None);
        ide.open_path(&txt);
        assert_eq!((ide.lang(), ide.title(window::Id::unique()).as_str()), (Lang::Plain, "notes.txt - Rust IDE"));
        ide.open_path(&md);
        assert_eq!((ide.lang(), ide.content.text().as_str()), (Lang::Plain, "# Title\n"));
        assert_eq!(ide.suggested_name(), "readme.md");
    }

    #[test]
    fn environment_items_open_the_files_and_the_listing() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        let items = menu_items(Menu::Tools, ide.toggles(), &[], 0);
        assert!(items.iter().any(|i| i.label == "General environment..."));
        send(&mut ide, [Message::EditEnv(true)]);
        let path = ide.path.clone().expect("the general file is open");
        assert!(path.ends_with("environment.env"), "{}", path.display());
        assert!(ide.content.text().contains("HTTP_PROXY"));
        send(&mut ide, [Message::ShowEnv]);
        assert!(ide.path.is_none());
        assert!(ide.content.text().contains("# General:") && ide.content.text().contains("# Project:"));
    }

    #[test]
    fn the_rust_toolchain_is_chosen_per_project_and_shown() {
        let _settings = isolate_settings();
        let root = std::env::temp_dir().join("rust-ide-gui-toolchain");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
        let mut ide = Ide::default();
        ide.path = Some(root.join("main.rs"));
        assert!(menu_items(Menu::Tools, ide.toggles(), &[], 0).iter().any(|i| i.label == "Rust toolchain..."));
        assert_eq!(ide.toolchain_label(), "  Rust default");

        // The picker: default first, installed ones after, "Other name..." last.
        let gnu = "stable-x86_64-pc-windows-gnu".to_string();
        let other = "nightly-x86_64-pc-windows-msvc".to_string();
        send(&mut ide, [Message::ToolchainsLoaded(vec![gnu.clone(), other.clone()])]);
        assert_eq!(ide.toolchains.as_ref().map(|d| (d.choices.len(), d.selected)), Some((3, 0)), "the default is current");
        let _ = toolchain_dialog(ide.toolchains.as_ref().unwrap());
        send(&mut ide, [Message::Nav(NavKey::Down), Message::Nav(NavKey::Enter)]);
        assert!(ide.toolchains.is_none(), "picking closes the dialog");
        assert_eq!(ide.toolchain_label(), format!("  Rust {gnu}"));
        send(&mut ide, [Message::ToolchainsLoaded(vec![gnu.clone(), other.clone()])]);
        assert_eq!(ide.toolchains.as_ref().map(|d| d.selected), Some(1), "starts on the current choice");
        send(&mut ide, [Message::Nav(NavKey::Char('o'))]);
        assert!(ide.toolchains.is_none() && ide.search.is_some(), "Other name opens the typed prompt");
        send(&mut ide, [Message::CloseSearch]);
        send(&mut ide, [Message::ToolchainsLoaded(vec![gnu.clone()]), Message::Escape]);
        assert!(ide.toolchains.is_none(), "Esc closes it");
        send(&mut ide, [Message::ToolchainsLoaded(vec![gnu.clone()]), Message::ToolchainPick(0)]);
        assert_eq!(ide.toolchain_label(), "  Rust default", "the Default row clears the choice");

        send(&mut ide, [Message::OpenSearch(SearchMode::Toolchain)]);
        send(&mut ide, [Message::SearchQuery("bad name!".into()), Message::SetToolchain]);
        assert!(ide.status.contains("not a toolchain name") && ide.search.is_some(), "{}", ide.status);
        send(&mut ide, [Message::SearchQuery("nightly".into()), Message::SetToolchain]);
        assert!(ide.search.is_none(), "the bar closes once it is set");
        let nightly = rust_ide::env::qualify_toolchain("nightly", cfg!(windows));
        assert_eq!(ide.toolchain_label(), format!("  Rust {nightly}"));
        assert!(std::fs::read_to_string(root.join(".rust-ide").join("env")).unwrap().contains(&format!("RUSTUP_TOOLCHAIN={nightly}")));

        send(&mut ide, [Message::OpenSearch(SearchMode::Toolchain)]);
        assert_eq!(ide.search.as_ref().map(|b| b.query.as_str()), Some(nightly.as_str()), "starts from the current choice");
        send(&mut ide, [Message::SearchQuery(String::new()), Message::SetToolchain]);
        assert_eq!(ide.toolchain_label(), "  Rust default");
        ide.path = Some(root.join("notes.py"));
        assert_eq!(ide.toolchain_label(), "", "only Rust files show it");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_open_files_and_cursors_come_back_in_the_next_session() {
        let _settings = isolate_settings();
        let root = std::env::temp_dir().join("rust-ide-gui-session");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let (a, b) = (root.join("a.rs"), root.join("b.py"));
        std::fs::write(&a, "fn main() {}\nlet x = 1;\n").unwrap();
        std::fs::write(&b, "print('héllo')\nprint(2)\n").unwrap();

        let mut ide = Ide::default();
        ide.persist_settings = true;
        assert!(ide.open_path(&a) && ide.open_path(&b));
        ide.place_clamped(0, "print('h".len() + 2); // after the "é" (2 bytes)
        let _ = ide.select_window(0);
        ide.place_clamped(1, 4);
        let snapshot = ide.session_snapshot();
        assert_eq!(snapshot.files.len(), 2);
        assert_eq!((snapshot.active, snapshot.files[0].line, snapshot.files[0].column), (0, 1, 4));
        assert_eq!((snapshot.files[1].line, snapshot.files[1].column), (0, 9), "columns are in characters");
        ide.save_session();

        let mut next = Ide::default();
        let session = rust_ide::session::startup(&root).expect("the session was saved");
        next.restore_session(&session);
        assert_eq!(next.window_count(), 2);
        assert_eq!(next.path.as_deref(), Some(a.as_path()), "the active window is back");
        assert_eq!((next.content.cursor().position.line, next.content.cursor().position.column), (1, 4));
        let _ = next.select_window(1);
        assert_eq!(next.content.cursor().position.column, "print('h".len() + 2, "converted back to bytes");

        // Without persistence (the tests) nothing is written.
        rust_ide::session::forget(&root);
        let mut quiet = Ide::default();
        quiet.open_path(&a);
        quiet.save_session();
        assert!(rust_ide::session::load(&root).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_chooser_dialog_builds_and_is_in_the_help() {
        let _settings = isolate_settings();
        let _ = language_dialog(&LanguageDialog { selected: 1, startup: true });
        let _ = language_dialog(&LanguageDialog { selected: 4, startup: false });
        assert!(SHORTCUTS.iter().any(|(_, keys, _)| keys.contains("R P C T M")), "listed in the F1 help");
    }
}

#[cfg(test)]
mod test_support {
    use super::*;

    /// Keeps tests away from the real settings file, and makes the tests that use
    /// it take turns (the settings file is shared by the whole test process).
    pub fn isolate_settings() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        static ONCE: std::sync::Once = std::sync::Once::new();
        let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        ONCE.call_once(|| {
            let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("gui-tests-settings.txt");
            let _ = std::fs::remove_file(&file);
            // SAFETY: set once, before any test reads settings.
            unsafe { std::env::set_var("RUST_IDE_CONFIG_FILE", file) };
        });
        guard
    }
}

#[cfg(test)]
mod window_style_tests {
    use super::*;
    use crate::test_support::isolate_settings;

    fn ide_with_windows(count: usize) -> Ide {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("style-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let mut ide = Ide::default();
        for n in 1..=count {
            let path = dir.join(format!("window{n}.rs"));
            std::fs::write(&path, format!("// window {n}\n")).unwrap();
            ide.open_path(&path);
        }
        ide
    }

    fn send(ide: &mut Ide, messages: impl IntoIterator<Item = Message>) {
        for message in messages {
            let _ = ide.update(message);
        }
    }

    #[test]
    fn setting_values_round_trip() {
        assert_eq!(WindowStyle::from_setting(None), WindowStyle::Horizontal, "today's look is the default");
        assert_eq!(WindowStyle::from_setting(Some("nonsense")), WindowStyle::Horizontal);
        for style in [WindowStyle::Legacy, WindowStyle::Horizontal, WindowStyle::Vertical] {
            assert_eq!(WindowStyle::from_setting(Some(style.setting())), style);
        }
        assert_eq!(WindowStyle::Vertical.setting(), "vertical");
    }

    #[test]
    fn choosing_a_style_applies_it_and_saves_it() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        ide.persist_settings = true;
        send(&mut ide, [Message::WindowStyle(WindowStyle::Vertical)]);
        assert_eq!(ide.window_style, WindowStyle::Vertical);
        assert_eq!(ide.status, "Window style: vertical tabs");
        assert_eq!(platform::read_setting("window_style").as_deref(), Some("vertical"));
        send(&mut ide, [Message::WindowStyle(WindowStyle::Legacy)]);
        assert_eq!(platform::read_setting("window_style").as_deref(), Some("legacy"));
        assert_eq!(WindowStyle::from_setting(platform::read_setting("window_style").as_deref()), WindowStyle::Legacy);
    }

    #[test]
    fn options_menu_lists_the_styles_with_radio_marks() {
        for current in [WindowStyle::Legacy, WindowStyle::Horizontal, WindowStyle::Vertical] {
            let toggles = Toggles { window_style: current, ..Toggles::default() };
            let items = menu_items(Menu::Window, toggles, &[], 0);
            let styles: Vec<_> = items
                .iter()
                .filter(|i| matches!(i.message, Some(Message::WindowStyle(_))))
                .collect();
            assert_eq!(styles.len(), 3);
            assert_eq!(styles.iter().map(|i| i.hotkey).collect::<String>(), "lht");
            for item in styles {
                let Some(Message::WindowStyle(style)) = item.message else { unreachable!() };
                assert!(item.label.starts_with(style.label()));
                assert_eq!(item.label.ends_with("(*)"), style == current, "{}", item.label);
            }
        }
        let hotkeys: Vec<char> = menu_items(Menu::Window, Toggles::default(), &[], 0).iter().map(|i| i.hotkey).collect();
        let mut unique = hotkeys.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), hotkeys.len(), "no hotkey is used twice in the Window menu: {hotkeys:?}");
    }

    #[test]
    fn menu_hotkeys_pick_a_style() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        send(&mut ide, [Message::OpenMenu(Menu::Window), Message::Nav(NavKey::Char('t'))]);
        assert_eq!(ide.window_style, WindowStyle::Vertical);
        assert!(ide.menu.is_none());
        send(&mut ide, [Message::OpenMenu(Menu::Window), Message::Nav(NavKey::Char('l'))]);
        assert_eq!(ide.window_style, WindowStyle::Legacy);
        send(&mut ide, [Message::OpenMenu(Menu::Window), Message::Nav(NavKey::Char('h'))]);
        assert_eq!(ide.window_style, WindowStyle::Horizontal);
    }

    #[test]
    fn every_style_builds_with_one_and_with_many_windows() {
        for style in [WindowStyle::Legacy, WindowStyle::Horizontal, WindowStyle::Vertical] {
            for count in [1, 3] {
                let mut ide = ide_with_windows(count);
                ide.window_style = style;
                let _ = ide.view(window::Id::unique());
                let _ = ide.window_tabs();
                let _ = ide.window_list();
                let _ = ide.legacy_title();
            }
        }
    }

    #[test]
    fn legacy_title_shows_the_name_and_the_window_number() {
        let (left, right) = legacy_title_text("main.rs*", 2, 3);
        assert!(left.contains("main.rs*"), "{left}");
        assert!(right.contains("2/3"), "{right}");
        let ide = ide_with_windows(3);
        assert_eq!(ide.window_names(), ["window1.rs", "window2.rs", "window3.rs"]);
        assert_eq!(ide.active, 2);
    }

    #[test]
    fn keys_work_the_same_in_every_style() {
        let _settings = isolate_settings();
        for style in [WindowStyle::Legacy, WindowStyle::Horizontal, WindowStyle::Vertical] {
            let mut ide = ide_with_windows(3);
            send(&mut ide, [Message::WindowStyle(style), Message::PrevWindow]);
            assert_eq!(ide.active, 1, "{style:?}");
            send(&mut ide, [Message::SelectWindow(0)]);
            assert_eq!(ide.active, 0, "{style:?}");
        }
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    fn send(ide: &mut Ide, messages: impl IntoIterator<Item = Message>) {
        for message in messages {
            let _ = ide.update(message);
        }
    }

    fn type_text(ide: &mut Ide, text: &str) {
        for c in text.chars() {
            let action = if c == '\n' { Edit::Enter } else { Edit::Insert(c) };
            send(ide, [Message::Edit(Action::Edit(action))]);
        }
    }

    fn markdown_window() -> Ide {
        let mut ide = Ide::default();
        ide.untitled_ext = Some("md");
        ide
    }

    #[test]
    fn the_preview_is_only_for_markdown_windows() {
        let mut ide = Ide::default();
        send(&mut ide, [Message::TogglePreview]);
        assert!(!ide.preview, "a Rust window has no preview");
        assert_eq!(ide.status, "The preview is for Markdown (.md) files");

        let mut ide = markdown_window();
        send(&mut ide, [Message::TogglePreview]);
        assert!(ide.preview && ide.preview_visible());
        assert_eq!(ide.status, "Markdown preview on");
        send(&mut ide, [Message::TogglePreview]);
        assert!(!ide.preview_visible());
        assert_eq!(ide.status, "Markdown preview off");
    }

    #[test]
    fn markdown_is_recognised_by_name_or_chosen_type() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("preview-tests");
        std::fs::create_dir_all(&dir).unwrap();
        for (name, expected) in [("a.md", true), ("b.MD", true), ("c.markdown", true), ("d.txt", false), ("e.rs", false)] {
            let path = dir.join(name);
            std::fs::write(&path, "x\n").unwrap();
            let mut ide = Ide::default();
            ide.open_path(&path);
            assert_eq!(ide.is_markdown(), expected, "{name}");
        }
        assert!(markdown_window().is_markdown());
        assert!(!Ide::default().is_markdown());
    }

    #[test]
    fn the_preview_follows_what_is_typed() {
        let mut ide = markdown_window();
        send(&mut ide, [Message::TogglePreview]);
        assert!(ide.preview_content.items().is_empty());
        type_text(&mut ide, "# Title");
        assert_eq!(ide.preview_content.items().len(), 1);
        type_text(&mut ide, "\n\nSome **bold** text\n\n- one\n- two");
        let kinds: Vec<&str> = ide
            .preview_content
            .items()
            .iter()
            .map(|item| match item {
                markdown::Item::Heading(..) => "heading",
                markdown::Item::Paragraph(_) => "paragraph",
                markdown::Item::List { .. } => "list",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, ["heading", "paragraph", "list"]);
        assert_eq!(ide.preview_source, ide.content.text());
        let _ = ide.view(window::Id::unique());
    }

    #[test]
    fn the_preview_shows_only_with_the_markdown_window_active() {
        let mut ide = markdown_window();
        send(&mut ide, [Message::TogglePreview]);
        type_text(&mut ide, "# Notes");
        // A second, Rust, window: no preview there, but it comes back on return.
        send(&mut ide, [Message::New, Message::NewFileType(0)]);
        assert_eq!(ide.window_count(), 2);
        assert!(!ide.preview_visible());
        send(&mut ide, [Message::SelectWindow(0)]);
        assert!(ide.preview_visible());
        assert_eq!(ide.preview_content.items().len(), 1, "the text is read again for this window");
        let _ = ide.view(window::Id::unique());
    }

    #[test]
    fn keys_and_menu_item() {
        let physical = key::Physical::Unidentified(key::NativeCode::Unidentified);
        assert!(matches!(
            shortcut(&Key::Named(key::Named::F6), physical, Modifiers::ALT),
            Some(Message::TogglePreview)
        ));
        assert!(matches!(
            shortcut(&Key::Named(key::Named::F6), physical, Modifiers::empty()),
            Some(Message::NextWindow)
        ));
        assert!(matches!(
            shortcut(&Key::Character("p".into()), physical, Modifiers::ALT),
            Some(Message::TogglePreview)
        ));
        let on = Toggles { preview: true, ..Toggles::default() };
        let items = menu_items(Menu::Window, on, &["a.md".to_string()], 0);
        let item = items.iter().find(|i| i.label.starts_with("Markdown preview")).expect("menu item");
        assert!(item.label.ends_with("[x]") && item.hotkey == 'm' && item.keys == "Alt+F6");
        let items = menu_items(Menu::Window, Toggles::default(), &["a.md".to_string()], 0);
        assert!(items.iter().any(|i| i.label.starts_with("Markdown preview") && i.label.ends_with("[ ]")));
        assert!(SHORTCUTS.iter().any(|(_, keys, _)| keys.contains("Alt+F6")));
    }

    #[test]
    fn links_open_in_the_browser_or_as_files_and_nothing_else() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("preview-links");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("other doc.md"), "# Other\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "plain\n").unwrap();
        let d = Some(dir.as_path());
        assert_eq!(classify_link("https://example.com/a?b=1", d), LinkTarget::Web("https://example.com/a?b=1".into()));
        assert_eq!(classify_link("HTTP://Example.com", d), LinkTarget::Web("HTTP://Example.com".into()));
        assert_eq!(classify_link("mailto:me@example.com", d), LinkTarget::Web("mailto:me@example.com".into()));
        assert_eq!(classify_link("#install", d), LinkTarget::Heading);
        assert_eq!(classify_link("notes.txt", d), LinkTarget::File(dir.join("notes.txt")));
        assert_eq!(classify_link("other%20doc.md#top", d), LinkTarget::File(dir.join("other doc.md")));
        assert_eq!(classify_link("missing.md", d), LinkTarget::Unknown);
        assert_eq!(classify_link("notes.txt", None), LinkTarget::Unknown, "an unsaved document has no folder");
        for refused in ["file:///C:/Windows/notepad.exe", "ms-settings:privacy", "javascript:alert(1)", "ftp://host/x"] {
            assert_eq!(classify_link(refused, d), LinkTarget::Unknown, "{refused}");
        }
    }

    #[test]
    fn clicking_a_file_link_opens_it_in_a_window() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("preview-links2");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.md"), "[next](next.md)\n").unwrap();
        std::fs::write(dir.join("next.md"), "# Next\n").unwrap();
        let mut ide = Ide::default();
        ide.open_path(&dir.join("main.md"));
        send(&mut ide, [Message::PreviewLink("next.md".into())]);
        assert_eq!(ide.window_names(), ["main.md", "next.md"]);
        assert_eq!(ide.content.text(), "# Next\n");
        send(&mut ide, [Message::PreviewLink("#top".into())]);
        assert_eq!(ide.status, "Links to a heading are not followed yet");
        send(&mut ide, [Message::PreviewLink("gone.md".into())]);
        assert_eq!(ide.status, "Cannot open the link: gone.md");
    }
}

#[cfg(test)]
mod settings_safety_tests {
    use super::*;
    use crate::test_support::isolate_settings;

    #[test]
    fn a_default_ide_never_writes_settings() {
        let _settings = isolate_settings();
        let file = platform::config_path().expect("settings path");
        let _ = std::fs::remove_file(&file);
        let mut ide = Ide::default();
        assert!(!ide.persist_settings);
        for message in [
            Message::WindowStyle(WindowStyle::Vertical),
            Message::ToggleCheckBefore,
            Message::ToggleVim,
            Message::ToggleStartupCheck(false),
            Message::New,
            Message::NewFileType(1),
        ] {
            let _ = ide.update(message);
        }
        assert!(!file.exists(), "no settings file was written");
        assert_eq!(ide.window_style, WindowStyle::Vertical, "the change itself still applies");
        assert_eq!(ide.untitled_ext, Some("py"));
    }

    #[test]
    fn the_real_program_writes_settings() {
        let _settings = isolate_settings();
        let file = platform::config_path().expect("settings path");
        let _ = std::fs::remove_file(&file);
        let mut ide = Ide::default();
        ide.persist_settings = true;
        let _ = ide.update(Message::ToggleCheckBefore);
        assert_eq!(platform::read_setting("check_before").as_deref(), Some("true"));
    }
}

#[cfg(test)]
mod font_tests {
    use super::*;
    use crate::test_support::isolate_settings;

    #[test]
    fn keys_change_the_size_inside_the_limits() {
        let mut ide = Ide::default();
        assert_eq!(ide.font_size(), FONT_DEFAULT);
        for _ in 0..40 {
            let _ = ide.update(Message::FontLarger);
        }
        assert_eq!(ide.font_size(), FONT_MAX);
        assert!(ide.status.contains("largest"), "{}", ide.status);
        for _ in 0..40 {
            let _ = ide.update(Message::FontSmaller);
        }
        assert_eq!(ide.font_size(), FONT_MIN);
        let _ = ide.update(Message::FontDefault);
        assert_eq!(ide.font_size(), FONT_DEFAULT);
        let _ = ide.view(window::Id::unique());
    }

    #[test]
    fn ctrl_equals_minus_and_zero_are_the_shortcuts() {
        let physical = key::Physical::Unidentified(key::NativeCode::Unidentified);
        let ctrl = Modifiers::CTRL;
        let key = |c: &str| shortcut(&Key::Character(c.into()), physical, ctrl);
        assert!(matches!(key("="), Some(Message::FontLarger)));
        assert!(matches!(key("+"), Some(Message::FontLarger)));
        assert!(matches!(key("-"), Some(Message::FontSmaller)));
        assert!(matches!(key("0"), Some(Message::FontDefault)));
    }

    #[test]
    fn options_menu_has_the_font_items_with_unique_hotkeys() {
        let items = menu_items(Menu::Options, Toggles::default(), &[], 0);
        for (label, hotkey, keys) in [("Font larger", 'l', "Ctrl+="), ("Font smaller", 'm', "Ctrl+-"), ("Font default", 'd', "Ctrl+0")] {
            let i = items.iter().find(|i| i.label == label).expect(label);
            assert!(i.hotkey == hotkey && i.keys == keys);
        }
        let mut hotkeys: Vec<char> = items.iter().map(|i| i.hotkey).collect();
        hotkeys.sort();
        hotkeys.dedup();
        assert_eq!(hotkeys.len(), items.len());
    }

    #[test]
    fn the_size_is_saved_and_a_bad_value_is_ignored() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        ide.persist_settings = true;
        let _ = ide.update(Message::FontLarger);
        assert_eq!(platform::read_setting("font_size").as_deref(), Some("17"));
        let _ = ide.update(Message::FontDefault);
        assert_eq!(platform::read_setting("font_size").as_deref(), Some("16"));
    }
}

#[cfg(test)]
mod preview_place_tests {
    use super::*;

    fn send(ide: &mut Ide, messages: impl IntoIterator<Item = Message>) {
        for message in messages {
            let _ = ide.update(message);
        }
    }

    fn type_text(ide: &mut Ide, text: &str) {
        for c in text.chars() {
            send(ide, [Message::Edit(Action::Edit(Edit::Insert(c)))]);
        }
    }

    fn markdown_window(mode: PreviewMode) -> Ide {
        let mut ide = Ide::default();
        ide.untitled_ext = Some("md");
        ide.preview_mode = mode;
        type_text(&mut ide, "# Hi");
        ide
    }

    #[test]
    fn the_mode_setting_round_trips_and_cycles() {
        for mode in [PreviewMode::Side, PreviewMode::Tab, PreviewMode::Window] {
            assert_eq!(PreviewMode::from_setting(Some(mode.setting())), mode);
        }
        assert_eq!(PreviewMode::from_setting(None), PreviewMode::Side);
        let mut ide = Ide::default();
        send(&mut ide, [Message::CyclePreviewMode]);
        assert_eq!(ide.preview_mode, PreviewMode::Tab);
        send(&mut ide, [Message::CyclePreviewMode, Message::CyclePreviewMode]);
        assert_eq!(ide.preview_mode, PreviewMode::Side);
        let items = menu_items(Menu::Window, ide.toggles(), &[], 0);
        assert!(items.iter().any(|i| i.label == "Preview in: side by side" && i.message.is_some()));
    }

    #[test]
    fn a_preview_tab_opens_follows_the_source_and_closes() {
        let mut ide = markdown_window(PreviewMode::Tab);
        send(&mut ide, [Message::TogglePreview]);
        assert_eq!(ide.window_count(), 2);
        assert_eq!(ide.active, 1);
        assert!(ide.preview_tab.is_some());
        assert_eq!(ide.window_names()[1], "Preview NONAME.MD");
        assert_eq!(ide.preview_source, "# Hi");
        assert!(ide.toggles().preview, "the Window menu shows the preview as on");

        // Nothing can be edited or saved in the preview tab.
        send(&mut ide, [Message::Edit(Action::Edit(Edit::Insert('x'))), Message::Save]);
        assert_eq!(ide.content.text(), "");
        assert!(ide.status.starts_with("This is a preview tab"));

        // The source changes; selecting the preview tab shows the new text.
        send(&mut ide, [Message::SelectWindow(0)]);
        type_text(&mut ide, " there");
        send(&mut ide, [Message::SelectWindow(1)]);
        assert_eq!(ide.preview_source, "# Hi there");

        // Alt+F6 inside the preview closes it and returns to the file.
        send(&mut ide, [Message::TogglePreview]);
        assert_eq!(ide.window_count(), 1);
        assert_eq!(ide.content.text(), "# Hi there");
        assert!(ide.preview_tab.is_none());

        // A second time it makes a new tab; with one open, it just goes there.
        send(&mut ide, [Message::TogglePreview, Message::SelectWindow(0), Message::TogglePreview]);
        assert_eq!(ide.window_count(), 2);
        assert_eq!(ide.active, 1);
    }

    #[test]
    fn the_tab_is_only_for_markdown_and_is_never_reused_as_a_blank_file() {
        let mut ide = Ide::default();
        ide.preview_mode = PreviewMode::Tab;
        send(&mut ide, [Message::TogglePreview]);
        assert_eq!(ide.window_count(), 1);
        assert_eq!(ide.status, "The preview is for Markdown (.md) files");

        let mut ide = markdown_window(PreviewMode::Tab);
        send(&mut ide, [Message::TogglePreview]);
        assert!(!ide.is_blank_window(), "a preview tab is not a blank window");
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("preview-place");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("other.md");
        std::fs::write(&path, "other\n").unwrap();
        ide.open_path(&path);
        assert_eq!(ide.window_count(), 3, "the file got its own window");
        assert!(ide.preview_tab.is_none());
    }

    #[test]
    fn the_preview_window_opens_follows_and_closes() {
        let mut ide = markdown_window(PreviewMode::Window);
        send(&mut ide, [Message::TogglePreview]);
        let id = ide.preview_window.expect("window requested");
        assert!(ide.preview_live());
        assert_eq!(ide.preview_source, "# Hi");
        assert!(ide.toggles().preview);
        type_text(&mut ide, "!");
        assert_eq!(ide.preview_source, "# Hi!", "follows what is typed");
        assert_eq!(ide.title(id), "Markdown preview - Rust IDE");
        assert!(!ide.preview_visible(), "nothing is drawn beside the editor");

        // The user closes the window with its X.
        send(&mut ide, [Message::WindowClosed(id)]);
        assert!(ide.preview_window.is_none());
        assert!(!ide.preview_live());

        // Alt+F6 opens it again, and again closes it.
        send(&mut ide, [Message::TogglePreview]);
        assert!(ide.preview_window.is_some());
        send(&mut ide, [Message::TogglePreview]);
        assert!(ide.preview_window.is_none());
    }

    #[test]
    fn changing_the_mode_closes_the_preview_window() {
        let mut ide = markdown_window(PreviewMode::Window);
        send(&mut ide, [Message::TogglePreview, Message::CyclePreviewMode]);
        assert_eq!(ide.preview_mode, PreviewMode::Side);
        assert!(ide.preview_window.is_none());
        assert!(!ide.preview);
    }
}

#[cfg(test)]
mod settings_dialog_tests {
    use super::*;
    use crate::test_support::isolate_settings;

    fn send(ide: &mut Ide, messages: impl IntoIterator<Item = Message>) {
        for message in messages {
            let _ = ide.update(message);
        }
    }

    fn keys(ide: &mut Ide, keys: impl IntoIterator<Item = NavKey>) {
        for key in keys {
            let _ = ide.update(Message::Nav(key));
        }
    }

    #[test]
    fn the_dialog_opens_from_the_options_menu_and_closes_with_escape() {
        let mut ide = Ide::default();
        let items = menu_items(Menu::Options, ide.toggles(), &[], 0);
        assert_eq!(items[0].label, "Settings...");
        send(&mut ide, [Message::OpenSettings]);
        assert!(ide.settings.is_some());
        let _ = ide.view(window::Id::unique());
        send(&mut ide, [Message::Escape]);
        assert!(ide.settings.is_none());
    }

    #[test]
    fn rows_change_the_settings_with_the_keys() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        ide.persist_settings = true;
        send(&mut ide, [Message::OpenSettings]);

        // Row 0: where the preview opens.
        keys(&mut ide, [NavKey::Right]);
        assert_eq!(ide.preview_mode, PreviewMode::Tab);
        keys(&mut ide, [NavKey::Left, NavKey::Left]);
        assert_eq!(ide.preview_mode, PreviewMode::Window);

        // Row 1: window style (Horizontal is the default).
        keys(&mut ide, [NavKey::Down, NavKey::Right]);
        assert_eq!(ide.window_style, WindowStyle::Vertical);
        keys(&mut ide, [NavKey::Left]);
        assert_eq!(ide.window_style, WindowStyle::Horizontal);

        // Rows 2 and 3: toggles.
        keys(&mut ide, [NavKey::Down, NavKey::Enter]);
        assert!(ide.vim.is_some());
        keys(&mut ide, [NavKey::Down, NavKey::Char(' ')]);
        assert!(ide.check_before);

        // Row 4: line numbers.
        keys(&mut ide, [NavKey::Down, NavKey::Enter]);
        assert!(ide.line_numbers);
        assert_eq!(platform::read_setting("line_numbers").as_deref(), Some("true"));

        // Row 5: font size.
        let before = ide.font_size();
        keys(&mut ide, [NavKey::Down, NavKey::Right]);
        assert_eq!(ide.font_size(), before + 1);
        keys(&mut ide, [NavKey::Left, NavKey::Left]);
        assert_eq!(ide.font_size(), before - 1);
        assert_eq!(platform::read_setting("preview_mode").as_deref(), Some("window"));
        assert_eq!(platform::read_setting("vim").as_deref(), Some("true"));

        // Rows 6 and 7: the mono theme and the two-line status bar (both put back at once).
        keys(&mut ide, [NavKey::Down, NavKey::Enter]);
        assert!(platform::mono());
        keys(&mut ide, [NavKey::Enter]);
        assert!(!platform::mono());
        keys(&mut ide, [NavKey::Down, NavKey::Enter]);
        assert!(ide.two_line_status);
        keys(&mut ide, [NavKey::Enter]);
        assert!(!ide.two_line_status);

        // Row 8 opens the dependencies check and leaves the dialog; the last row closes it.
        keys(&mut ide, [NavKey::Down, NavKey::Enter]);
        assert!(ide.settings.is_none());
        send(&mut ide, [Message::OpenSettings]);
        keys(&mut ide, [NavKey::End, NavKey::Enter]);
        assert!(ide.settings.is_none());
    }

    #[test]
    fn line_numbers_have_a_menu_item_a_setting_and_a_view() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        let label = |ide: &Ide| {
            menu_items(Menu::Options, ide.toggles(), &[], 0)
                .into_iter()
                .map(|i| i.label.to_string())
                .find(|l| l.starts_with("Line numbers"))
                .expect("menu item")
        };
        assert!(label(&ide).ends_with("[ ]"));
        let _ = ide.view(window::Id::unique());
        ide.persist_settings = true;
        let _ = ide.update(Message::ToggleLineNumbers);
        assert!(ide.line_numbers);
        assert!(label(&ide).ends_with("[x]"));
        assert_eq!(platform::read_setting("line_numbers").as_deref(), Some("true"));
        ide.content = text_editor::Content::with_text("one\nTwo\nthree\n");
        let _ = ide.view(window::Id::unique());
        let _ = ide.update(Message::ToggleLineNumbers);
        assert!(!ide.line_numbers);
        assert_eq!(platform::read_setting("line_numbers").as_deref(), Some("false"));
    }

    #[test]
    fn letters_and_clicks_use_a_row_and_keys_do_not_leak_to_the_editor() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        send(&mut ide, [Message::OpenSettings]);
        keys(&mut ide, [NavKey::Char('v')]);
        assert!(ide.vim.is_some());
        send(&mut ide, [Message::SettingsAct(3, 0)]);
        assert!(ide.check_before);
        assert_eq!(ide.settings.as_ref().map(|d| d.selected), Some(3));
        keys(&mut ide, [NavKey::Char('x')]);
        assert_eq!(ide.content.text(), "");
        let _ = ide.view(window::Id::unique());
    }
}

#[cfg(test)]
mod project_dialog_tests {
    use super::*;
    use crate::test_support::isolate_settings;
    use rust_ide::templates::{Lang, Size};

    fn send(ide: &mut Ide, messages: impl IntoIterator<Item = Message>) {
        for message in messages {
            let _ = ide.update(message);
        }
    }

    fn keys(ide: &mut Ide, keys: impl IntoIterator<Item = NavKey>) {
        for key in keys {
            let _ = ide.update(Message::Nav(key));
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("gui-project-test").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_file_menu_lists_it_and_escape_closes_it() {
        let mut ide = Ide::default();
        let items = menu_items(Menu::File, ide.toggles(), &[], 0);
        assert!(items.iter().any(|i| i.label == "New project..."));
        send(&mut ide, [Message::OpenProject]);
        assert!(ide.project.is_some());
        let _ = ide.view(window::Id::unique());
        send(&mut ide, [Message::Escape]);
        assert!(ide.project.is_none());
    }

    #[test]
    fn keys_choose_language_and_size_and_move_between_fields() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        send(&mut ide, [Message::OpenProject]);
        keys(&mut ide, [NavKey::Char('p')]);
        assert_eq!(ide.project.as_ref().unwrap().lang, Lang::Python);
        keys(&mut ide, [NavKey::Left]);
        assert_eq!(ide.project.as_ref().unwrap().lang, Lang::Rust);
        keys(&mut ide, [NavKey::Down, NavKey::Char('c')]);
        assert_eq!(ide.project.as_ref().unwrap().size, Size::Complex);
        keys(&mut ide, [NavKey::Right]);
        assert_eq!(ide.project.as_ref().unwrap().size, Size::Simple, "wraps round");
        keys(&mut ide, [NavKey::Down]);
        assert_eq!(ide.project.as_ref().unwrap().row, PROJECT_NAME);
        // In a text field the letters belong to the text, not to the dialog.
        keys(&mut ide, [NavKey::Char('p'), NavKey::Left]);
        assert_eq!(ide.project.as_ref().unwrap().lang, Lang::Rust);
        keys(&mut ide, [NavKey::Up, NavKey::Up]);
        assert_eq!(ide.project.as_ref().unwrap().row, PROJECT_LANG);
        let _ = ide.view(window::Id::unique());
    }

    #[test]
    fn a_csharp_project_is_created_and_its_program_opens_highlighted() {
        let _settings = isolate_settings();
        let parent = scratch("create-cs");
        let mut ide = Ide::default(); // does not save settings, so the other tests' starting choices stay put
        send(&mut ide, [Message::OpenProject]);
        keys(&mut ide, [NavKey::Char('c')]);
        assert_eq!(ide.project.as_ref().unwrap().lang, Lang::CSharp);
        send(
            &mut ide,
            [
                Message::ProjectFolder(parent.to_string_lossy().into_owned()),
                Message::ProjectName("cs-tool".into()),
                Message::ProjectAct(PROJECT_SIZE, 1),
                Message::ProjectAct(PROJECT_SIZE, 1),
                Message::ProjectCreate,
            ],
        );
        let main = parent.join("cs-tool").join("src").join("cs-tool").join("Program.cs");
        assert!(main.is_file(), "the Medium layout");
        assert_eq!(ide.path.as_deref(), Some(main.as_path()));
        assert_eq!(ide.lang(), highlight::Lang::CSharp);
        assert!(ide.content.text().contains("cs_tool.Core"), "the namespace uses underscores");
        assert!(parent.join("cs-tool").join("cs-tool.slnx").is_file());
    }

    #[test]
    fn create_makes_the_project_and_opens_its_main_file() {
        let _settings = isolate_settings();
        let parent = scratch("create");
        let mut ide = Ide::default();
        ide.persist_settings = true;
        send(&mut ide, [Message::OpenProject]);
        send(
            &mut ide,
            [
                Message::ProjectFolder(parent.to_string_lossy().into_owned()),
                Message::ProjectName("my-tool".into()),
                Message::ProjectAct(PROJECT_LANG, 1),
                Message::ProjectAct(PROJECT_SIZE, 1),
                Message::ProjectCreate,
            ],
        );
        assert!(ide.project.is_none());
        let main = parent.join("my-tool").join("my_tool").join("__main__.py");
        assert!(main.is_file());
        assert_eq!(ide.path.as_deref(), Some(main.as_path()));
        assert!(ide.content.text().contains("greet"));
        assert_eq!(templates::last_lang(), Lang::Python);
        assert_eq!(templates::last_size(), Size::Low);
    }

    #[test]
    fn a_bad_name_or_existing_folder_keeps_the_dialog_open_with_a_reason() {
        let _settings = isolate_settings();
        let parent = scratch("bad");
        let mut ide = Ide::default();
        send(&mut ide, [Message::OpenProject, Message::ProjectFolder(parent.to_string_lossy().into_owned())]);
        send(&mut ide, [Message::ProjectName("1abc".into()), Message::ProjectCreate]);
        assert!(ide.project.as_ref().unwrap().error.contains("letter"));
        send(&mut ide, [Message::ProjectName("ok".into())]);
        assert!(ide.project.as_ref().unwrap().error.is_empty(), "typing clears the message");
        send(&mut ide, [Message::ProjectCreate, Message::OpenProject]);
        send(
            &mut ide,
            [Message::ProjectFolder(parent.to_string_lossy().into_owned()), Message::ProjectName("ok".into()), Message::ProjectCreate],
        );
        assert!(ide.project.as_ref().unwrap().error.contains("already exists"));
        let _ = ide.view(window::Id::unique());
    }
}

#[cfg(test)]
mod menu_layout_tests {
    use super::*;

    #[test]
    fn menus_group_their_items_and_never_repeat_a_hotkey() {
        let labels = |menu| -> Vec<String> {
            menu_items(menu, Toggles::default(), &[], 0).into_iter().map(|i| i.label).collect()
        };
        let has = |menu, word: &str| labels(menu).iter().any(|l| l.starts_with(word));
        for word in ["Dependencies", "General environment", "Project environment", "Show environment", "Python .venv", "Rust toolchain"] {
            assert!(has(Menu::Tools, word) && !has(Menu::Options, word), "{word} belongs in Tools");
        }
        for word in ["Settings", "Vim mode", "Check before run", "Line numbers", "Font larger"] {
            assert!(has(Menu::Options, word), "{word} belongs in Options");
        }
        for word in ["Markdown preview", "Preview in", "Legacy windows", "Horizontal tabs", "Vertical tabs"] {
            assert!(has(Menu::Window, word) && !has(Menu::Options, word), "{word} belongs in Window");
        }
        for menu in MENUS {
            let mut hotkeys: Vec<char> = menu_items(menu, Toggles::default(), &[], 0).iter().map(|i| i.hotkey).collect();
            let all = hotkeys.len();
            hotkeys.sort();
            hotkeys.dedup();
            assert_eq!(hotkeys.len(), all, "{menu:?} repeats a hotkey");
        }
        let firsts: Vec<char> = MENU_TITLES.iter().filter_map(|t| t.chars().next()).collect();
        let mut unique = firsts.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), firsts.len(), "Alt+letter must open one menu: {firsts:?}");
    }
}
#[cfg(test)]
mod theme_tests {
    use super::test_support::isolate_settings;
    use super::*;

    #[test]
    fn mono_theme_and_two_line_status_are_options_with_settings() {
        let _settings = isolate_settings();
        let mut ide = Ide::default();
        ide.persist_settings = true;
        let items = menu_items(Menu::Options, ide.toggles(), &[], 0);
        assert!(items.iter().any(|i| i.label.starts_with("Mono theme") && i.label.ends_with("[ ]")));
        assert!(items.iter().any(|i| i.label.starts_with("Two-line status")));
        let turbo = turbo_blue();
        let _ = ide.update(Message::ToggleMono);
        assert!(platform::mono());
        assert_ne!(turbo_blue(), turbo, "the palette changes");
        assert_eq!(platform::read_setting("theme").as_deref(), Some("mono"));
        let _ = ide.view(window::Id::unique());
        let _ = ide.update(Message::ToggleStatusLines);
        assert!(ide.two_line_status);
        assert!(status_bar_height() > 30.0);
        assert_eq!(platform::read_setting("status_lines").as_deref(), Some("2"));
        let _ = ide.view(window::Id::unique());
        let _ = ide.update(Message::ToggleMono);
        let _ = ide.update(Message::ToggleStatusLines);
        assert!(!platform::mono() && !ide.two_line_status);
        assert_eq!(platform::read_setting("theme").as_deref(), Some("turbo"));
        assert!(status_bar_height() < 30.0);
    }
}