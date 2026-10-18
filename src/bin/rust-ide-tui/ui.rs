//! Draws the terminal IDE in Turbo Pascal colors.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use rust_ide::filetype;
use rust_ide::highlight::{Highlight, LineHighlighter};

use crate::app::{App, MENUS, Menu, Mode, PromptKind};

const BAR: Style = Style::new().fg(Color::Black).bg(Color::Gray);
const HOTKEY: Style = Style::new().fg(Color::Red).bg(Color::Gray);
const CHOSEN: Style = Style::new().fg(Color::Black).bg(Color::Green);
const EDITOR: Style = Style::new().fg(Color::LightYellow).bg(Color::Blue);
const PANEL: Style = Style::new().fg(Color::Black).bg(Color::Cyan);
const MESSAGES_HEIGHT: u16 = 6;

/// Rows used by everything except the editor (for scrolling and paging).
pub fn editor_area(app: &App, screen: Rect) -> Rect {
    layout(app, screen)[2]
}

fn layout(app: &App, screen: Rect) -> [Rect; 5] {
    let tabs = u16::from(app.windows.len() > 1);
    let messages = if app.messages.is_empty() { 0 } else { MESSAGES_HEIGHT };
    Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(tabs),
        Constraint::Min(1),
        Constraint::Length(messages),
        Constraint::Length(1),
    ])
    .areas(screen)
}

pub fn draw(frame: &mut Frame, app: &App) {
    let [menu_bar, tabs, editor, messages, status] = layout(app, frame.area());
    draw_menu_bar(frame, app, menu_bar);
    if tabs.height > 0 {
        draw_tabs(frame, app, tabs);
    }
    draw_editor(frame, app, editor);
    if messages.height > 0 {
        draw_messages(frame, app, messages);
    }
    draw_status(frame, app, status);

    match &app.mode {
        Mode::Edit => {}
        Mode::Menu { menu, item } => draw_dropdown(frame, app, *menu, *item, menu_bar),
        Mode::Prompt { kind, input } => draw_prompt(frame, *kind, input),
        Mode::Open(dialog) => draw_open(frame, dialog),
        Mode::Help { scroll } => draw_help(frame, *scroll),
        Mode::Language { selected, startup } => draw_language(frame, *selected, *startup),
    }
}

fn title_x(menu: Menu) -> u16 {
    MENUS.iter().take_while(|&&m| m != menu).map(|m| m.title().len() as u16 + 3).sum::<u16>() + 1
}

fn hotkey_spans(label: &str, hotkey: char, normal: Style, hot: Style) -> Vec<Span<'static>> {
    match label.char_indices().find(|(_, c)| c.eq_ignore_ascii_case(&hotkey)) {
        Some((i, c)) => {
            let end = i + c.len_utf8();
            vec![
                Span::styled(label[..i].to_string(), normal),
                Span::styled(label[i..end].to_string(), hot),
                Span::styled(label[end..].to_string(), normal),
            ]
        }
        None => vec![Span::styled(label.to_string(), normal)],
    }
}

fn draw_menu_bar(frame: &mut Frame, app: &App, area: Rect) {
    let open = match app.mode {
        Mode::Menu { menu, .. } => Some(menu),
        _ => None,
    };
    let mut spans = vec![Span::styled(" ", BAR)];
    for menu in MENUS {
        let (normal, hot) = if open == Some(menu) { (CHOSEN, CHOSEN.fg(Color::Red)) } else { (BAR, HOTKEY) };
        spans.push(Span::styled(" ", normal));
        spans.extend(hotkey_spans(menu.title(), menu.title().chars().next().unwrap_or(' '), normal, hot));
        spans.push(Span::styled(" ", normal));
        spans.push(Span::styled(" ", BAR));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(BAR), area);
}

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = Vec::new();
    for (index, window) in app.windows.iter().enumerate() {
        let star = if window.modified { "*" } else { "" };
        let style = if index == app.active { PANEL } else { Style::new().fg(Color::DarkGray).bg(Color::Gray) };
        spans.push(Span::styled(format!(" {} {}{star} ", index + 1, window.name()), style));
        spans.push(Span::styled(" ", BAR));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(BAR), area);
}

fn color(highlight: Highlight) -> Color {
    match highlight {
        Highlight::Keyword => Color::White,
        Highlight::Type => Color::LightGreen,
        Highlight::Str => Color::LightCyan,
        Highlight::Number => Color::LightMagenta,
        Highlight::Comment => Color::Gray,
        Highlight::Attribute => Color::LightYellow,
        Highlight::Error => Color::LightRed,
        Highlight::Warning => Color::Magenta,
        Highlight::Match | Highlight::Current => Color::White,
        Highlight::Breakpoint => Color::LightRed,
        Highlight::ExecLine => Color::LightGreen,
    }
}

fn draw_editor(frame: &mut Frame, app: &App, area: Rect) {
    let buffer = app.buffer();
    let mut highlighter = LineHighlighter::new(buffer.lang());
    let width = area.width as usize;
    let mut lines = Vec::with_capacity(area.height as usize);
    // Lines above the view are still lexed, for multi-line comments/strings.
    for (row, text) in buffer.lines.iter().enumerate().take(buffer.scroll + area.height as usize) {
        let tokens = highlighter.line(text);
        if row < buffer.scroll {
            continue;
        }
        let mut styles = vec![EDITOR; text.len()];
        for (range, kind) in tokens {
            styles[range].fill(EDITOR.fg(color(kind)));
        }
        let mut spans: Vec<Span> = Vec::new();
        for (char_index, (byte, c)) in text.char_indices().enumerate() {
            if char_index < buffer.hscroll {
                continue;
            }
            if char_index >= buffer.hscroll + width {
                break;
            }
            let c = if c == '\t' { ' ' } else { c };
            spans.push(Span::styled(c.to_string(), styles[byte]));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines).style(EDITOR), area);

    if matches!(app.mode, Mode::Edit) {
        let x = area.x + (buffer.char_col() - buffer.hscroll) as u16;
        let y = area.y + (buffer.row - buffer.scroll) as u16;
        frame.set_cursor_position(Position::new(x, y));
    }
}

fn draw_messages(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![Line::styled(" Messages   Alt+F8 next  Alt+F7 previous", PANEL.fg(Color::White))];
    let first = app.message_index.unwrap_or(0).saturating_sub(area.height as usize - 2);
    for (index, m) in app.messages.iter().enumerate().skip(first).take(area.height as usize - 1) {
        let place = match (&m.file, m.line) {
            (Some(file), line) if line > 0 => {
                format!("{}({line},{}): ", file.file_name().unwrap_or_default().to_string_lossy(), m.column)
            }
            _ => String::new(),
        };
        let style = if app.message_index == Some(index) {
            CHOSEN
        } else if m.level == "error" {
            PANEL.fg(Color::Red)
        } else {
            PANEL
        };
        lines.push(Line::styled(format!(" {} {place}{}", m.level.to_uppercase(), m.message), style));
    }
    frame.render_widget(Paragraph::new(lines).style(PANEL), area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let buffer = app.buffer();
    let position = format!(" {}:{} ", buffer.row + 1, buffer.char_col() + 1);
    let keys = " F1 Help  F2 Save  F3 Open  F5 Run  F9 Compile  F10 Menu ";
    let room = (area.width as usize).saturating_sub(keys.len() + position.len());
    let status: String = app.status.chars().take(room).collect();
    let line = Line::from(vec![
        Span::styled(keys, BAR),
        Span::styled(format!("{status:<room$}"), BAR.fg(Color::Red)),
        Span::styled(position, BAR),
    ]);
    frame.render_widget(Paragraph::new(line).style(BAR), area);
}

/// A Turbo-style double-bordered dialog box.
fn dialog(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Double)
        .title(format!(" {title} "))
        .style(Style::new().fg(Color::White).bg(Color::Gray))
        .border_style(Style::new().fg(Color::White).bg(Color::Gray))
        .title_style(Style::new().fg(Color::White).bg(Color::Gray).add_modifier(Modifier::BOLD))
}

fn centered(screen: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(screen.width);
    let height = height.min(screen.height);
    Rect::new(screen.x + (screen.width - width) / 2, screen.y + (screen.height - height) / 2, width, height)
}

fn draw_dropdown(frame: &mut Frame, app: &App, menu: Menu, selected: usize, bar: Rect) {
    let items = app.menu_items(menu);
    let width = items.iter().map(|i| i.label.chars().count() + i.keys.len() + 6).max().unwrap_or(10) as u16;
    let area = Rect::new(bar.x + title_x(menu), bar.y + 1, width, items.len() as u16 + 2).intersection(frame.area());
    frame.render_widget(Clear, area);
    let block = Block::bordered().style(BAR).border_style(BAR);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let (normal, hot) = if index == selected { (CHOSEN, CHOSEN.fg(Color::Red)) } else { (BAR, HOTKEY) };
            let mut spans = vec![Span::styled(" ", normal)];
            spans.extend(hotkey_spans(&item.label, item.hotkey, normal, hot));
            let pad = (inner.width as usize).saturating_sub(item.label.chars().count() + item.keys.len() + 2);
            spans.push(Span::styled(format!("{}{} ", " ".repeat(pad), item.keys), normal));
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_prompt(frame: &mut Frame, kind: PromptKind, input: &str) {
    let title = match kind {
        PromptKind::SaveAs => "Save file as",
        PromptKind::Find => "Find",
        PromptKind::GoTo => "Go to line number",
    };
    let area = centered(frame.area(), 60, 5);
    frame.render_widget(Clear, area);
    let block = dialog(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [field, hint] = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(inner);
    frame.render_widget(Paragraph::new(format!(" {input}")).style(EDITOR.fg(Color::White)), field);
    let hint_text = match kind {
        PromptKind::SaveAs => " Enter OK  Esc Cancel  (no extension: file type is added)",
        _ => " Enter OK   Esc Cancel",
    };
    frame.render_widget(Paragraph::new(hint_text).style(BAR), hint);
    frame.set_cursor_position(Position::new(field.x + 1 + input.chars().count() as u16, field.y));
}

fn draw_open(frame: &mut Frame, dialog_state: &crate::app::OpenDialog) {
    let area = centered(frame.area(), 70, 20);
    frame.render_widget(Clear, area);
    let block = dialog("Open a file");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [dir, field, list, hint] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    frame.render_widget(Paragraph::new(format!(" {}", dialog_state.dir.display())).style(BAR), dir);
    frame.render_widget(Paragraph::new(format!(" Name: {}", dialog_state.filter)).style(EDITOR.fg(Color::White)), field);
    let visible = dialog_state.visible();
    let first = dialog_state.selected.saturating_sub(list.height as usize - 1);
    let lines: Vec<Line> = visible
        .iter()
        .enumerate()
        .skip(first)
        .take(list.height as usize)
        .map(|(index, (name, is_dir))| {
            let label = if *is_dir { format!(" {name}\\") } else { format!(" {name}") };
            let style = if index == dialog_state.selected { CHOSEN } else { PANEL };
            Line::styled(format!("{label:<width$}", width = list.width as usize), style)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(PANEL), list);
    frame.render_widget(Paragraph::new(" Type to filter  Up/Down  Enter open  Backspace up  Esc").style(BAR), hint);
    frame.set_cursor_position(Position::new(field.x + 7 + dialog_state.filter.chars().count() as u16, field.y));
}

/// The shortcuts of the terminal IDE, shown by F1.
pub const SHORTCUTS: &[(&str, &str)] = &[
    ("F10 / Alt+letter", "Open a menu; arrows, Enter or the red letter choose"),
    ("F1", "This help (Up/Down/PgUp/PgDn scroll, Esc closes)"),
    ("F2 / Ctrl+S", "Save (asks for a name the first time)"),
    ("F3 / Ctrl+O", "Open a file (each in its own window)"),
    ("File > New", "Asks for the language: R Rust, P Python, T Text, M Markdown (or 1-4)"),
    ("Alt+X / Ctrl+Q", "Exit (press twice to discard changes)"),
    ("Ctrl+Z / Ctrl+Y", "Undo / redo"),
    ("Tab / Shift+Tab", "Indent / unindent"),
    ("Ctrl+Left/Right", "Previous / next word"),
    ("Ctrl+Home/End", "Start / end of the file"),
    ("Ctrl+F", "Find"),
    ("Ctrl+L", "Search again"),
    ("Ctrl+G", "Go to line number"),
    ("F9", "Compile the current file"),
    ("Alt+F9", "Syntax check: find errors without building or running"),
    ("Alt+F8 / Alt+F7", "Next / previous error"),
    ("F5 / Ctrl+F9", "Save, compile and run in the terminal"),
    ("Alt+F5", "User screen: the program's output"),
    ("F6 / Shift+F6", "Next / previous window"),
    ("Alt+1 ... Alt+9", "Go to window 1-9; Alt+0 window list"),
    ("Alt+F3", "Close the window"),
];

fn draw_language(frame: &mut Frame, selected: usize, startup: bool) {
    let title = if startup { "Choose a language" } else { "New file: choose a language" };
    let area = centered(frame.area(), 64, 11);
    frame.render_widget(Clear, area);
    let block = dialog(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [list, note, hint] =
        Layout::vertical([Constraint::Length(filetype::FILE_TYPES.len() as u16 + 1), Constraint::Length(2), Constraint::Length(1)])
            .areas(inner);
    let lines: Vec<Line> = filetype::FILE_TYPES
        .iter()
        .enumerate()
        .map(|(index, kind)| {
            let (normal, hot) = if index == selected { (CHOSEN, CHOSEN.fg(Color::Red)) } else { (BAR, HOTKEY) };
            let mut spans = vec![Span::styled(" ", normal)];
            spans.extend(hotkey_spans(kind.label, kind.hotkey, normal, hot));
            let pad = (list.width as usize).saturating_sub(kind.label.chars().count() + kind.ext.len() + 3);
            spans.push(Span::styled(format!("{}.{} ", " ".repeat(pad), kind.ext), normal));
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(BAR), list);
    frame.render_widget(Paragraph::new(format!(" {}", filetype::NOTE)).style(BAR.fg(Color::DarkGray)).wrap(ratatui::widgets::Wrap { trim: true }), note);
    let escape = if startup { "Esc: plain Rust window" } else { "Esc: cancel" };
    frame.render_widget(Paragraph::new(format!(" Up/Down  Enter OK  R P T M / 1-4  {escape}")).style(BAR), hint);
}

fn draw_help(frame: &mut Frame, scroll: usize) {
    let area = centered(frame.area(), 76, 22);
    frame.render_widget(Clear, area);
    let block = dialog("Keyboard shortcuts");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines: Vec<Line> = SHORTCUTS
        .iter()
        .skip(scroll.min(SHORTCUTS.len().saturating_sub(1)))
        .map(|(keys, action)| {
            Line::from(vec![
                Span::styled(format!(" {keys:<20}"), PANEL.fg(Color::White)),
                Span::styled(action.to_string(), PANEL),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(PANEL), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn screen(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| (0..width).map(|x| buffer[(x, y)].symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn draws_menu_editor_and_status() {
        let mut app = App::new(&[]);
        for c in "fn main() {}".chars() {
            app.buffer_mut().insert_char(c);
        }
        let text = screen(&app, 80, 10);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].contains("File") && lines[0].contains("Search") && lines[0].contains("Help"));
        assert!(lines[1].starts_with("fn main() {}"));
        assert!(lines[9].contains("F1 Help") && lines[9].trim_end().ends_with("1:13"));
    }

    #[test]
    fn draws_the_language_chooser() {
        let mut app = App::new(&[]);
        app.ask_language(true);
        let text = screen(&app, 80, 20);
        for expected in ["Choose a language", "Rust", "Python", "Text file", "Markdown", ".rs", ".md", "Rust and Python are supported"] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        assert!(text.contains("Esc: plain Rust window"));
        app.ask_language(false);
        let text = screen(&app, 80, 20);
        assert!(text.contains("New file: choose a language") && text.contains("Esc: cancel"));
    }

    #[test]
    fn draws_open_menu_and_messages() {
        let mut app = App::new(&[]);
        app.mode = Mode::Menu { menu: Menu::Search, item: 1 };
        app.messages = vec![rust_ide::runner::Diagnostic {
            file: None,
            line: 0,
            column: 0,
            level: "error".into(),
            message: "something broke".into(),
        }];
        let text = screen(&app, 80, 16);
        assert!(text.contains("Find..."));
        assert!(text.contains("Search again"));
        assert!(text.contains("ERROR something broke"));
    }
}
