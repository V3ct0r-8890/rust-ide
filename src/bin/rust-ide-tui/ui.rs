//! Draws the terminal IDE in Turbo Pascal colors.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use rust_ide::filetype;
use rust_ide::highlight::{Highlight, LineHighlighter};

use crate::app::{
    App, MENUS, Menu, Mode, PROJECT_CREATE, PROJECT_FOLDER, PROJECT_LANG, PROJECT_NAME, PROJECT_SIZE, ProjectDialog, PromptKind,
};

// Fixed RGB colors (the classic Turbo palette), so the contrast does not depend on
// how the terminal theme maps the 16 ANSI colors.
const BLUE: Color = Color::Rgb(0, 0, 170);
const SILVER: Color = Color::Rgb(192, 192, 192);
const TEAL: Color = Color::Rgb(0, 170, 170);
const GREEN: Color = Color::Rgb(0, 170, 0);
const DARK_RED: Color = Color::Rgb(140, 0, 0);
const DARK_BLUE: Color = Color::Rgb(0, 0, 100);
const BAR: Style = Style::new().fg(Color::Black).bg(SILVER);
const HOTKEY: Style = Style::new().fg(DARK_RED).bg(SILVER);
const CHOSEN: Style = Style::new().fg(Color::Black).bg(GREEN);
const EDITOR: Style = Style::new().fg(Color::Rgb(255, 255, 85)).bg(BLUE);
const CURSOR: Style = Style::new().fg(Color::Rgb(0, 0, 170)).bg(Color::Rgb(255, 255, 85));
const PANEL: Style = Style::new().fg(Color::Black).bg(TEAL);
const GUTTER: Style = Style::new().fg(Color::Rgb(160, 175, 255)).bg(DARK_BLUE);
const MESSAGES_HEIGHT: u16 = 6;

/// Rows used by everything except the editor (for scrolling and paging).
pub fn editor_area(app: &App, screen: Rect) -> Rect {
    text_area(app, layout(app, screen)[2])
}

/// Width of the line number column (digits plus a space), 0 when it is off.
fn gutter_width(app: &App) -> u16 {
    if app.line_numbers { app.buffer().lines.len().to_string().len() as u16 + 1 } else { 0 }
}

/// The part of the editor area that holds text, right of the line numbers.
fn text_area(app: &App, area: Rect) -> Rect {
    let gutter = gutter_width(app).min(area.width.saturating_sub(1));
    Rect::new(area.x + gutter, area.y, area.width - gutter, area.height)
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
        Mode::Project(dialog) => draw_project(frame, dialog),
    }
}

/// The menu whose title is at column `x` of the menu bar.
pub fn menu_at(x: u16) -> Option<Menu> {
    MENUS.iter().copied().find(|&m| x >= title_x(m) - 1 && x < title_x(m) + m.title().len() as u16 + 1)
}

/// The window whose tab is at column `x` of the tab row.
pub fn tab_at(app: &App, x: u16) -> Option<usize> {
    let mut start = 0u16;
    for (index, window) in app.windows.iter().enumerate() {
        let star = u16::from(window.modified);
        let width = format!(" {} ", index + 1).len() as u16 + window.name().chars().count() as u16 + star + 1;
        if x >= start && x < start + width {
            return Some(index);
        }
        start += width + 1;
    }
    None
}

fn dropdown_area(app: &App, menu: Menu, screen: Rect) -> Rect {
    let items = app.menu_items(menu);
    let width = items.iter().map(|i| i.label.chars().count() + i.keys.len() + 6).max().unwrap_or(10) as u16;
    Rect::new(screen.x + title_x(menu), screen.y + 1, width, items.len() as u16 + 2).intersection(screen)
}

/// The item of an open menu under the screen position, if any.
pub fn dropdown_item_at(app: &App, menu: Menu, screen: Rect, x: u16, y: u16) -> Option<usize> {
    let area = dropdown_area(app, menu, screen);
    let inside = x > area.x && x + 1 < area.right() && y > area.y && y + 1 < area.bottom();
    inside.then(|| (y - area.y - 1) as usize).filter(|&i| i < app.menu_items(menu).len())
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
        let (normal, hot) = if open == Some(menu) { (CHOSEN, CHOSEN.fg(DARK_RED)) } else { (BAR, HOTKEY) };
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
        let style = if index == app.active { PANEL } else { Style::new().fg(Color::Rgb(60, 60, 60)).bg(SILVER) };
        spans.push(Span::styled(format!(" {} {}{star} ", index + 1, window.name()), style));
        spans.push(Span::styled(" ", BAR));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(BAR), area);
}

fn color(highlight: Highlight) -> Color {
    match highlight {
        Highlight::Keyword => Color::White,
        Highlight::Type => Color::Rgb(85, 255, 85),
        Highlight::Str => Color::Rgb(85, 255, 255),
        Highlight::Number => Color::Rgb(255, 135, 255),
        Highlight::Comment => Color::Rgb(160, 175, 255),
        Highlight::Attribute => Color::Rgb(255, 255, 85),
        Highlight::Error => Color::Rgb(255, 100, 100),
        Highlight::Warning => Color::Rgb(255, 170, 0),
        Highlight::Match | Highlight::Current => Color::White,
        Highlight::Breakpoint => Color::Rgb(255, 100, 100),
        Highlight::ExecLine => Color::Rgb(85, 255, 85),
    }
}

fn draw_editor(frame: &mut Frame, app: &App, full: Rect) {
    let buffer = app.buffer();
    let area = text_area(app, full);
    if area.x > full.x {
        let digits = (area.x - full.x) as usize - 1;
        let numbers: Vec<Line> = (buffer.scroll..buffer.scroll + area.height as usize)
            .map(|row| {
                let text = if row < buffer.lines.len() { format!("{:>digits$} ", row + 1) } else { " ".repeat(digits + 1) };
                Line::styled(text, if row == buffer.row { GUTTER.fg(Color::White) } else { GUTTER })
            })
            .collect();
        frame.render_widget(Paragraph::new(numbers).style(GUTTER), Rect::new(full.x, full.y, area.x - full.x, full.height));
    }
    let mut highlighter = LineHighlighter::new(buffer.lang());
    let width = area.width as usize;
    // The cursor is drawn as a colored block (the terminal's own cursor color
    // may be unreadable on the blue editor), so the real cursor stays hidden.
    let show_cursor = matches!(app.mode, Mode::Edit);
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
            let style = if show_cursor && row == buffer.row && char_index == buffer.char_col() { CURSOR } else { styles[byte] };
            spans.push(Span::styled(c.to_string(), style));
        }
        if show_cursor && row == buffer.row {
            let len = text.chars().count();
            if buffer.char_col() >= len && buffer.char_col() < buffer.hscroll + width {
                spans.push(Span::styled(" ", CURSOR));
            }
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines).style(EDITOR), area);

}

fn draw_messages(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![Line::styled(" Messages   Alt+F8 next  Alt+F7 previous", PANEL.fg(DARK_BLUE))];
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
            PANEL.fg(DARK_RED)
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
        Span::styled(format!("{status:<room$}"), BAR.fg(DARK_RED)),
        Span::styled(position, BAR),
    ]);
    frame.render_widget(Paragraph::new(line).style(BAR), area);
}

/// A Turbo-style double-bordered dialog box.
fn dialog(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Double)
        .title(format!(" {title} "))
        .style(Style::new().fg(Color::Black).bg(SILVER))
        .border_style(Style::new().fg(Color::Black).bg(SILVER))
        .title_style(Style::new().fg(Color::Black).bg(SILVER).add_modifier(Modifier::BOLD))
}

fn centered(screen: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(screen.width);
    let height = height.min(screen.height);
    Rect::new(screen.x + (screen.width - width) / 2, screen.y + (screen.height - height) / 2, width, height)
}

fn draw_dropdown(frame: &mut Frame, app: &App, menu: Menu, selected: usize, bar: Rect) {
    let items = app.menu_items(menu);
    let area = dropdown_area(app, menu, frame.area());
    let _ = bar;
    frame.render_widget(Clear, area);
    let block = Block::bordered().style(BAR).border_style(BAR);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let (normal, hot) = if index == selected { (CHOSEN, CHOSEN.fg(DARK_RED)) } else { (BAR, HOTKEY) };
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
        PromptKind::ReplaceFind => "Replace: text to find",
        PromptKind::ReplaceWith => "Replace with",
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
    ("File > New project", "Rust or Python, size Simple/Low/Medium/Complex, a name and a folder (Tab moves, Enter creates)"),
    ("Alt+X / Ctrl+Q", "Exit (press twice to discard changes)"),
    ("Ctrl+Z / Ctrl+Y", "Undo / redo"),
    ("Tab / Shift+Tab", "Indent / unindent"),
    ("Ctrl+Left/Right", "Previous / next word"),
    ("Ctrl+Home/End", "Start / end of the file"),
    ("Ctrl+F", "Find"),
    ("Ctrl+H / Ctrl+R", "Replace all (asks for the text, then the replacement; Ctrl+Z undoes)"),
    ("Ctrl+L", "Search again"),
    ("Options > Line numbers", "Show or hide the line numbers (remembered)"),
    ("Options > Check before run", "F9 / F5 look for errors first and stop if there are any (remembered)"),
    ("Mouse", "Click a menu, a menu item, a tab or a place in the text; the wheel moves the cursor (start with --no-mouse to select text with the mouse)"),
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

fn draw_project(frame: &mut Frame, state: &ProjectDialog) {
    let area = centered(frame.area(), 72, 16);
    frame.render_widget(Clear, area);
    let block = dialog("New project");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = [
        (PROJECT_LANG, "Language", 'l', format!("< {} >", state.lang.label())),
        (PROJECT_SIZE, "Size", 's', format!("< {} >", state.size.label())),
        (PROJECT_NAME, "Name", 'n', state.name.clone()),
        (PROJECT_FOLDER, "Folder", 'f', state.folder.clone()),
    ];
    let width = inner.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    let mut cursor = None;
    for (index, label, hotkey, value) in rows {
        let (normal, hot) = if index == state.row { (CHOSEN, CHOSEN.fg(DARK_RED)) } else { (BAR, HOTKEY) };
        let mut spans = vec![Span::styled(" ", normal)];
        spans.extend(hotkey_spans(&format!("{label:<9}"), hotkey, normal, hot));
        let text_row = index == PROJECT_NAME || index == PROJECT_FOLDER;
        let room = width.saturating_sub(12);
        // A long text shows its end, where the typing happens.
        let shown: String = if value.chars().count() > room {
            value.chars().skip(value.chars().count() - room).collect()
        } else {
            value.clone()
        };
        let used = 10 + shown.chars().count();
        if text_row && index == state.row {
            cursor = Some(Position::new(inner.x + used as u16, inner.y + lines.len() as u16));
        }
        spans.push(Span::styled(format!("{shown}{}", " ".repeat(width.saturating_sub(used))), normal));
        lines.push(Line::from(spans));
    }
    for (index, label, hotkey) in [(PROJECT_CREATE, "Create", 'c'), (PROJECT_CREATE + 1, "Cancel", '\0')] {
        let (normal, hot) = if index == state.row { (CHOSEN, CHOSEN.fg(DARK_RED)) } else { (BAR, HOTKEY) };
        let mut spans = vec![Span::styled(" ", normal)];
        spans.extend(hotkey_spans(label, hotkey, normal, hot));
        spans.push(Span::styled(" ".repeat(width.saturating_sub(label.len() + 1)), normal));
        lines.push(Line::from(spans));
    }
    let target = std::path::Path::new(state.folder.trim()).join(state.name.trim());
    let problem = match rust_ide::templates::check_name(&state.name) {
        _ if !state.error.is_empty() => state.error.clone(),
        Err(why) if !state.name.is_empty() => why,
        _ => String::new(),
    };
    lines.push(Line::from(""));
    lines.push(Line::from(format!(" {}", state.size.describe(state.lang))));
    lines.push(Line::from(format!(" Creates {}", target.display())));
    lines.push(Line::from(Span::styled(format!(" {problem}"), BAR.fg(DARK_RED))));
    lines.push(Line::from(" Tab/Up/Down field  Left/Right or letters change  Enter Create  Esc Cancel"));
    frame.render_widget(Paragraph::new(lines).style(BAR), inner);
    if let Some(position) = cursor {
        frame.set_cursor_position(position);
    }
}

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
            let (normal, hot) = if index == selected { (CHOSEN, CHOSEN.fg(DARK_RED)) } else { (BAR, HOTKEY) };
            let mut spans = vec![Span::styled(" ", normal)];
            spans.extend(hotkey_spans(kind.label, kind.hotkey, normal, hot));
            let pad = (list.width as usize).saturating_sub(kind.label.chars().count() + kind.ext.len() + 3);
            spans.push(Span::styled(format!("{}.{} ", " ".repeat(pad), kind.ext), normal));
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(BAR), list);
    frame.render_widget(Paragraph::new(format!(" {}", filetype::NOTE)).style(BAR.fg(Color::Rgb(60, 60, 60))).wrap(ratatui::widgets::Wrap { trim: true }), note);
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
                Span::styled(format!(" {keys:<20}"), PANEL.fg(DARK_BLUE)),
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
    fn draws_the_new_project_dialog() {
        let mut app = App::new(&[]);
        app.ask_project();
        let text = screen(&app, 90, 24);
        for expected in ["New project", "Language", "< Rust >", "Size", "Name", "Folder", "Create", "Cancel", "Creates"] {
            assert!(text.contains(expected), "missing {expected:?} in
{text}");
        }
    }

    #[test]
    fn draws_line_numbers_when_on() {
        let mut app = App::new(&[]);
        for c in ['a', '\n', 'b', '\n', 'c'] {
            if c == '\n' { app.buffer_mut().newline() } else { app.buffer_mut().insert_char(c) }
        }
        assert!(screen(&app, 40, 8).lines().nth(1).unwrap().starts_with('a'));
        app.line_numbers = true;
        let text = screen(&app, 40, 8);
        let rows: Vec<&str> = text.lines().collect();
        assert!(rows[1].starts_with("1 a") && rows[2].starts_with("2 b") && rows[3].starts_with("3 c"), "{text}");
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
