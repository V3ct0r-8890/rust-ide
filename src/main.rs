//! rust-ide: a Turbo Pascal–style IDE for Rust and Python (iced GUI front end).

mod runner;

use std::path::{Path, PathBuf};

use iced::keyboard::{self, Key, Modifiers, key};
use iced::widget::text_editor::{Binding, Cursor, KeyPress, Position};
use iced::widget::{
    Space, button, column, container, mouse_area, row, scrollable, stack, text, text_editor,
};
use iced::{
    Background, Color, Element, Font, Length, Padding, Subscription, Task, Theme,
};

use runner::{BuildResult, Diagnostic};

const TURBO_BLUE: Color = Color::from_rgb(0.0, 0.0, 0.66);
const TURBO_CYAN: Color = Color::from_rgb(0.0, 0.66, 0.66);
const TURBO_GRAY: Color = Color::from_rgb(0.75, 0.75, 0.75);
const TURBO_GREEN: Color = Color::from_rgb(0.0, 0.66, 0.0);
const TURBO_YELLOW: Color = Color::from_rgb(1.0, 1.0, 0.33);
const TURBO_RED: Color = Color::from_rgb(0.66, 0.0, 0.0);

const MENU_TITLES: [&str; 9] = [
    "File", "Edit", "Search", "Run", "Compile", "Debug", "Options", "Window", "Help",
];
const MENU_TITLE_WIDTH: f32 = 84.0;
const MENU_BAR_HEIGHT: f32 = 26.0;

fn main() -> iced::Result {
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
}

/// What to do once the file is saved (Save As may be pending).
#[derive(Debug, Clone, Copy, PartialEq)]
enum AfterSave {
    Nothing,
    Compile,
    Run,
}

/// Commands that throw away unsaved edits and therefore need a second press.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Discard {
    New,
    Open,
    Exit,
}

#[derive(Default)]
struct Ide {
    content: text_editor::Content,
    path: Option<PathBuf>,
    modified: bool,
    menu: Option<Menu>,
    confirm: Option<Discard>,
    building: bool,
    diagnostics: Vec<Diagnostic>,
    status: String,
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
}

impl Ide {
    /// Starts empty, or with the file given on the command line.
    fn new() -> Self {
        let mut ide = Self::default();
        if let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) {
            if path.exists() {
                ide.load(&path);
            } else {
                ide.status = format!("New file {}", path.display());
                ide.path = Some(path);
            }
        }
        ide
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
        keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                ..
            } => shortcut(&key, physical_key, modifiers),
            _ => None,
        })
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        if !matches!(message, Message::Edit(_)) {
            self.menu = None;
        }
        let confirm = self.confirm.take();

        match message {
            Message::Edit(action) => {
                if action.is_edit() {
                    self.modified = true;
                }
                self.content.perform(action);
                Task::none()
            }
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
                self.status = "New file".into();
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
                self.save(after)
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
            Message::BuildFinished(result, after) => {
                self.building = false;
                self.finish_build(result, after);
                Task::none()
            }
            Message::JumpTo(index) => {
                if let Some(diagnostic) = self.diagnostics.get(index).cloned() {
                    self.jump_to(&diagnostic);
                }
                Task::none()
            }
        }
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
                self.status = format!("Opened {}", path.display());
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
        }
        self.status = format!("Saved {}", path.display());

        if after == AfterSave::Nothing || self.building {
            return Task::none();
        }
        self.building = true;
        self.diagnostics.clear();
        self.status = format!("Compiling {} ...", path.display());
        Task::perform(async move { runner::build(&path) }, move |result| {
            Message::BuildFinished(result, after)
        })
    }

    fn finish_build(&mut self, result: BuildResult, after: AfterSave) {
        self.diagnostics = result.diagnostics;
        let errors = self.diagnostics.iter().filter(|d| d.level == "error").count();
        let warnings = self.diagnostics.len() - errors;

        if !result.success {
            self.status = format!("Compile failed: {errors} error(s), {warnings} warning(s)");
            if let Some(first) = self.diagnostics.iter().find(|d| d.level == "error").cloned() {
                self.jump_to(&first);
            }
            return;
        }

        self.status = format!("Compile successful, {warnings} warning(s)");
        if after != AfterSave::Run {
            return;
        }
        if let Some(target) = result.target {
            self.status = match runner::run(&target) {
                Ok(()) if target.gui => "Running (GUI window)".into(),
                Ok(()) => "Running (console window)".into(),
                Err(e) => format!("Cannot run {}: {e}", target.program.display()),
            };
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
        let canonical = |p: &Path| std::fs::canonicalize(p).ok();
        self.path
            .as_deref()
            .is_some_and(|current| canonical(current) == canonical(file))
    }

    fn view(&self) -> Element<'_, Message> {
        let menu_bar = container(row(MENU_TITLES.iter().map(|&title| {
            let menu = match title {
                "File" => Some(Menu::File),
                "Run" => Some(Menu::Run),
                "Compile" => Some(Menu::Compile),
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

        let editor = text_editor(&self.content)
            .on_action(Message::Edit)
            .height(Length::Fill)
            .key_binding(|press: KeyPress| {
                shortcut(&press.key, press.physical_key, press.modifiers)
                    .map(Binding::Custom)
                    .or_else(|| Binding::from_key_press(press))
            })
            .style(|theme, status| text_editor::Style {
                background: Background::Color(TURBO_BLUE),
                value: TURBO_YELLOW,
                ..text_editor::default(theme, status)
            });

        let mut layout = column![menu_bar, editor];
        if !self.diagnostics.is_empty() {
            layout = layout.push(self.messages_panel());
        }
        layout = layout.push(self.status_bar());

        match self.menu {
            Some(menu) => stack![
                layout,
                mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                    .on_press(Message::CloseMenu),
                dropdown(menu),
            ]
            .into(),
            None => layout.into(),
        }
    }

    fn messages_panel(&self) -> Element<'_, Message> {
        let items = self.diagnostics.iter().enumerate().map(|(index, d)| {
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
            text(" Messages ").color(Color::WHITE),
            scrollable(column(items)).height(Length::Fill),
        ])
        .height(140)
        .width(Length::Fill)
        .style(|_| container::Style::default().background(TURBO_CYAN))
        .into()
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let cursor = self.content.cursor().position;
        let keys = " F2 Save  F3 Open  F5 Run  F9 Compile  Alt+X Exit";
        container(row![
            text(keys).color(Color::BLACK),
            text(format!("   {}", self.status)).color(TURBO_RED),
            Space::new().width(Length::Fill),
            text(format!("{}:{} ", cursor.line + 1, cursor.column + 1)).color(Color::BLACK),
        ])
        .padding([2, 4])
        .width(Length::Fill)
        .style(|_| container::Style::default().background(TURBO_GRAY))
        .into()
    }
}

/// Global shortcuts, Turbo Pascal style (plus the usual Ctrl+S / Ctrl+O).
fn shortcut(key: &Key, physical: key::Physical, modifiers: Modifiers) -> Option<Message> {
    match key.as_ref() {
        Key::Named(key::Named::F2) => Some(Message::Save),
        Key::Named(key::Named::F3) => Some(Message::Open),
        Key::Named(key::Named::F5) => Some(Message::Run),
        Key::Named(key::Named::F9) if modifiers.control() => Some(Message::Run),
        Key::Named(key::Named::F9) => Some(Message::Compile),
        _ => match key.to_latin(physical) {
            Some('x') if modifiers.alt() => Some(Message::Exit),
            Some('s') if modifiers.command() => Some(Message::Save),
            Some('o') if modifiers.command() => Some(Message::Open),
            _ => None,
        },
    }
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
        Menu::Run => (3, &[("Run", "F5", Message::Run)]),
        Menu::Compile => (4, &[("Compile", "F9", Message::Compile)]),
    };

    let entries = items.iter().map(|(label, keys, message)| {
        button(row![
            text(*label).color(Color::BLACK),
            Space::new().width(Length::Fill),
            text(*keys).color(Color::BLACK),
        ])
        .width(200)
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

fn menu_title_style(open: bool, status: button::Status) -> button::Style {
    let highlighted = open || matches!(status, button::Status::Hovered);
    button::Style {
        background: highlighted.then(|| Background::Color(TURBO_GREEN)),
        text_color: Color::BLACK,
        ..button::Style::default()
    }
}
