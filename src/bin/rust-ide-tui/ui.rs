//! Draws the terminal IDE in the Turbo Pascal, Mono or Modern colors.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use rust_ide::filetype;
use rust_ide::highlight::{Highlight, LineHighlighter};

use crate::app::{
    Action, App, DepsDialog, MENUS, Menu, MenuItem, Mode, PROJECT_CREATE, PROJECT_FOLDER, PROJECT_LANG, PROJECT_NAME, PROJECT_SIZE,
    ProjectDialog, PromptKind, SETTINGS_ROWS, WindowStyle,
};

// Fixed RGB colors, so the contrast does not depend on how the terminal theme maps
// the 16 ANSI colors. Turbo is the classic Turbo Pascal palette; Mono uses grays only;
// Modern is light, like a normal desktop program.
#[derive(Clone, Copy, PartialEq)]
enum Theme {
    Turbo,
    Mono,
    Modern,
}

fn theme() -> Theme {
    if rust_ide::platform::mono() {
        Theme::Mono
    } else if rust_ide::platform::modern() {
        Theme::Modern
    } else {
        Theme::Turbo
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// Menus, dialogs and the status bar.
fn silver() -> Color {
    match theme() {
        Theme::Turbo => rgb(192, 192, 192),
        Theme::Mono => rgb(200, 200, 200),
        Theme::Modern => rgb(240, 240, 240),
    }
}

/// Errors, the status message and hotkey letters.
fn dark_red() -> Color {
    match theme() {
        Theme::Turbo => rgb(140, 0, 0),
        Theme::Mono => Color::Black,
        Theme::Modern => rgb(170, 0, 0),
    }
}

/// Headings on the panels.
fn dark_blue() -> Color {
    match theme() {
        Theme::Turbo => rgb(0, 0, 100),
        Theme::Mono => Color::Black,
        Theme::Modern => rgb(0, 60, 140),
    }
}

fn bar() -> Style {
    Style::new().fg(Color::Black).bg(silver())
}

fn hot_style() -> Style {
    match theme() {
        Theme::Turbo => bar().fg(dark_red()),
        Theme::Mono => bar().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        Theme::Modern => bar().fg(rgb(0, 84, 166)).add_modifier(Modifier::UNDERLINED),
    }
}

/// The chosen menu item or row.
fn chosen() -> Style {
    match theme() {
        Theme::Turbo => Style::new().fg(Color::Black).bg(rgb(0, 170, 0)),
        Theme::Mono => Style::new().fg(Color::White).bg(rgb(70, 70, 70)),
        Theme::Modern => Style::new().fg(Color::White).bg(rgb(0, 120, 215)),
    }
}

/// The hotkey letter of the chosen item.
fn chosen_hot() -> Style {
    match theme() {
        Theme::Turbo => chosen().fg(dark_red()),
        _ => chosen().add_modifier(Modifier::UNDERLINED),
    }
}

fn editor() -> Style {
    match theme() {
        Theme::Turbo => Style::new().fg(rgb(255, 255, 85)).bg(rgb(0, 0, 170)),
        Theme::Mono => Style::new().fg(rgb(200, 200, 200)).bg(Color::Black),
        Theme::Modern => Style::new().fg(Color::Black).bg(Color::White),
    }
}

/// A text field in a dialog.
fn input_style() -> Style {
    match theme() {
        Theme::Modern => Style::new().fg(Color::Black).bg(Color::White),
        _ => editor().fg(Color::White),
    }
}

fn cursor() -> Style {
    match theme() {
        Theme::Turbo => Style::new().fg(rgb(0, 0, 170)).bg(rgb(255, 255, 85)),
        Theme::Mono => Style::new().fg(Color::Black).bg(Color::White),
        Theme::Modern => Style::new().fg(Color::White).bg(Color::Black),
    }
}

/// Selected text.
fn selected() -> Style {
    match theme() {
        Theme::Turbo => Style::new().fg(rgb(0, 0, 170)).bg(rgb(192, 192, 192)),
        Theme::Mono => Style::new().fg(Color::Black).bg(rgb(150, 150, 150)),
        Theme::Modern => Style::new().fg(Color::Black).bg(rgb(158, 199, 245)),
    }
}

fn panel() -> Style {
    match theme() {
        Theme::Turbo => Style::new().fg(Color::Black).bg(rgb(0, 170, 170)),
        Theme::Mono => Style::new().fg(Color::Black).bg(rgb(170, 170, 170)),
        Theme::Modern => Style::new().fg(Color::Black).bg(rgb(225, 225, 225)),
    }
}

fn gutter() -> Style {
    match theme() {
        Theme::Turbo => Style::new().fg(rgb(160, 175, 255)).bg(rgb(0, 0, 100)),
        Theme::Mono => Style::new().fg(rgb(140, 140, 140)).bg(rgb(30, 30, 30)),
        Theme::Modern => Style::new().fg(rgb(120, 120, 120)).bg(rgb(245, 245, 245)),
    }
}

/// The number of the cursor's line.
fn gutter_current() -> Style {
    gutter().fg(if theme() == Theme::Modern { Color::Black } else { Color::White })
}

/// Gray text on the dialogs.
fn dim() -> Style {
    bar().fg(rgb(60, 60, 60))
}

/// Matches of the search bar.
fn match_style() -> Style {
    match theme() {
        Theme::Turbo => Style::new().fg(Color::Black).bg(rgb(0, 170, 170)),
        Theme::Mono => Style::new().fg(Color::Black).bg(rgb(110, 110, 110)),
        Theme::Modern => Style::new().fg(Color::Black).bg(rgb(255, 230, 120)),
    }
}

/// A line with a breakpoint (Turbo Pascal's red bar).
fn breakpoint_line() -> Style {
    match theme() {
        Theme::Turbo => Style::new().fg(Color::White).bg(rgb(170, 0, 0)),
        Theme::Mono => Style::new().fg(Color::White).bg(rgb(80, 80, 80)),
        Theme::Modern => Style::new().fg(Color::Black).bg(rgb(255, 200, 200)),
    }
}

/// The line where the debugger stopped.
fn exec_line() -> Style {
    match theme() {
        Theme::Turbo => Style::new().fg(Color::Black).bg(rgb(0, 170, 170)),
        Theme::Mono => Style::new().fg(Color::Black).bg(rgb(200, 200, 200)),
        Theme::Modern => Style::new().fg(Color::Black).bg(rgb(255, 255, 150)),
    }
    .add_modifier(Modifier::BOLD)
}

const MESSAGES_HEIGHT: u16 = 6;
/// Width of the list of windows (Vertical tabs).
const WINDOW_LIST_WIDTH: u16 = 24;

/// Rows used by everything except the editor (for scrolling and paging).
pub fn editor_area(app: &App, screen: Rect) -> Rect {
    text_area(app, regions(app, screen).editor)
}

/// The places on the screen.
struct Regions {
    menu: Rect,
    /// The tabs (Horizontal) or the window title line (Legacy).
    windows: Rect,
    search: Rect,
    /// The list of windows (Vertical).
    list: Rect,
    editor: Rect,
    preview: Rect,
    /// Messages, or the debugger's watches, locals and call stack.
    panel: Rect,
    status: Rect,
}

fn debug_panel_shown(app: &App) -> bool {
    app.debug.is_some() || !app.watches.is_empty()
}

fn regions(app: &App, screen: Rect) -> Regions {
    let several = app.windows.len() > 1;
    let tab_row = match app.window_style {
        WindowStyle::Legacy => 1,
        WindowStyle::Horizontal => u16::from(several),
        WindowStyle::Vertical => 0,
    };
    let panel = if debug_panel_shown(app) || !app.panel_items().is_empty() { MESSAGES_HEIGHT } else { 0 };
    let [menu, windows, search, body, panel, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(tab_row),
        Constraint::Length(u16::from(app.search.is_some())),
        Constraint::Min(1),
        Constraint::Length(panel),
        Constraint::Length(if app.two_line_status { 2 } else { 1 }),
    ])
    .areas(screen);
    let list_width = if app.window_style == WindowStyle::Vertical && several { WINDOW_LIST_WIDTH.min(body.width / 3) } else { 0 };
    let [list, main] = Layout::horizontal([Constraint::Length(list_width), Constraint::Min(1)]).areas(body);
    let (editor, preview) = if app.preview_visible() {
        let [left, right] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(main);
        (left, right)
    } else {
        (main, Rect::new(main.right(), main.y, 0, main.height))
    };
    Regions { menu, windows, search, list, editor, preview, panel, status }
}

/// The window whose tab (or row of the window list) is under the mouse.
pub fn window_at(app: &App, screen: Rect, x: u16, y: u16) -> Option<usize> {
    let r = regions(app, screen);
    if app.window_style == WindowStyle::Horizontal && r.windows.height > 0 && y == r.windows.y {
        return tab_at(app, x);
    }
    if r.list.contains(Position::new(x, y)) {
        let index = (y - r.list.y) as usize + list_first(app, r.list);
        return (index < app.windows.len()).then_some(index);
    }
    None
}

/// The first window shown in the list, so the active one is in view.
fn list_first(app: &App, list: Rect) -> usize {
    app.active.saturating_sub((list.height as usize).saturating_sub(1))
}

/// The area of the Output window (over everything between the menu and the status bar).
pub fn output_area(app: &App, screen: Rect) -> Rect {
    let r = regions(app, screen);
    Rect::new(screen.x, r.menu.bottom(), screen.width, r.status.y.saturating_sub(r.menu.bottom()))
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

pub fn draw(frame: &mut Frame, app: &App) {
    let r = regions(app, frame.area());
    let menu_bar = r.menu;
    let editor = r.editor;
    draw_menu_bar(frame, app, menu_bar);
    if r.windows.height > 0 {
        if app.window_style == WindowStyle::Legacy {
            draw_legacy_title(frame, app, r.windows);
        } else {
            draw_tabs(frame, app, r.windows);
        }
    }
    if r.list.width > 0 {
        draw_window_list(frame, app, r.list);
    }
    if r.search.height > 0 {
        draw_search(frame, app, r.search);
    }
    match app.preview_source() {
        Some(source) => draw_preview(frame, source, editor, app.buffer().scroll),
        None if app.buffer().preview_of.is_some() => {
            frame.render_widget(Paragraph::new(" The file of this preview was closed (Alt+F6 closes it)").style(editor_style()), editor);
        }
        None => draw_editor(frame, app, editor),
    }
    if r.preview.width > 0 {
        draw_preview(frame, app.buffer(), r.preview, 0);
    }
    if r.panel.height > 0 {
        if debug_panel_shown(app) {
            draw_debug(frame, app, r.panel);
        } else {
            draw_messages(frame, app, r.panel);
        }
    }
    draw_status(frame, app, r.status);
    if app.output.visible {
        draw_output(frame, app, output_area(app, frame.area()));
    }

    match &app.mode {
        Mode::Edit if !app.output.visible => draw_completion(frame, app, editor),
        Mode::Edit => {}
        Mode::Menu { menu, item } => draw_dropdown(frame, app, *menu, *item, menu_bar),
        Mode::Context { item, .. } => {
            let area = context_area(app, frame.area()).unwrap_or_default();
            draw_menu_box(frame, &crate::app::context_items(), *item, area);
        }
        Mode::Settings { selected } => draw_settings(frame, app, *selected),
        Mode::Deps(dialog) => draw_deps(frame, dialog, app.check_at_startup),
        Mode::Prompt { kind, input } => draw_prompt(frame, *kind, input),
        Mode::Open(dialog) => draw_open(frame, dialog),
        Mode::Help { scroll } => draw_help(frame, *scroll),
        Mode::Language { selected, startup } => draw_language(frame, *selected, *startup),
        Mode::Toolchains { choices, selected } => draw_toolchains(frame, choices, *selected),
        Mode::Project(dialog) => draw_project(frame, dialog),
        Mode::Unsaved { action, button } => draw_unsaved(frame, app, action, *button),
        Mode::Lessons { track, selected } => draw_lessons(frame, *track, *selected),
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

fn menu_width(items: &[MenuItem]) -> u16 {
    items.iter().map(|i| i.label.chars().count() + i.keys.len() + 6).max().unwrap_or(10) as u16
}

fn dropdown_area(app: &App, menu: Menu, screen: Rect) -> Rect {
    let items = app.menu_items(menu);
    Rect::new(screen.x + title_x(menu), screen.y + 1, menu_width(&items), items.len() as u16 + 2).intersection(screen)
}

/// Where the right-click menu is: at the mouse, moved left or up to fit on the screen.
fn context_area(app: &App, screen: Rect) -> Option<Rect> {
    let Mode::Context { x, y, .. } = app.mode else { return None };
    let items = crate::app::context_items();
    let (width, height) = (menu_width(&items).min(screen.width), (items.len() as u16 + 2).min(screen.height));
    let x = x.min(screen.right() - width);
    let y = y.min(screen.bottom() - height);
    Some(Rect::new(x, y, width, height))
}

/// The item of the right-click menu under the screen position, if any.
pub fn context_item_at(app: &App, screen: Rect, x: u16, y: u16) -> Option<usize> {
    let area = context_area(app, screen)?;
    let inside = x > area.x && x + 1 < area.right() && y > area.y && y + 1 < area.bottom();
    inside.then(|| (y - area.y - 1) as usize).filter(|&i| i < crate::app::context_items().len())
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
    let mut spans = vec![Span::styled(" ", bar())];
    for menu in MENUS {
        let (normal, hot) = if open == Some(menu) { (chosen(), chosen_hot()) } else { (bar(), hot_style()) };
        spans.push(Span::styled(" ", normal));
        spans.extend(hotkey_spans(menu.title(), menu.title().chars().next().unwrap_or(' '), normal, hot));
        spans.push(Span::styled(" ", normal));
        spans.push(Span::styled(" ", bar()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(bar()), area);
}

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = Vec::new();
    for (index, window) in app.windows.iter().enumerate() {
        let star = if window.modified { "*" } else { "" };
        let style = if index == app.active { panel() } else { Style::new().fg(Color::Rgb(60, 60, 60)).bg(silver()) };
        spans.push(Span::styled(format!(" {} {}{star} ", index + 1, window.name()), style));
        spans.push(Span::styled(" ", bar()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(bar()), area);
}

fn color(highlight: Highlight) -> Color {
    match theme() {
        Theme::Turbo => {}
        Theme::Mono => {
            return match highlight {
                Highlight::Keyword | Highlight::Match | Highlight::Current | Highlight::Error => Color::White,
                Highlight::Type | Highlight::Attribute => rgb(225, 225, 225),
                Highlight::Str | Highlight::Number => rgb(175, 175, 175),
                Highlight::Comment => rgb(125, 125, 125),
                Highlight::Warning | Highlight::Breakpoint | Highlight::ExecLine => rgb(235, 235, 235),
            };
        }
        Theme::Modern => {
            return match highlight {
                Highlight::Keyword => rgb(0, 0, 200),
                Highlight::Type => rgb(0, 120, 120),
                Highlight::Str => rgb(163, 21, 21),
                Highlight::Number => rgb(9, 134, 88),
                Highlight::Comment => rgb(0, 128, 0),
                Highlight::Attribute => rgb(128, 100, 0),
                Highlight::Error | Highlight::Breakpoint => rgb(200, 0, 0),
                Highlight::Warning => rgb(180, 110, 0),
                Highlight::Match | Highlight::Current => Color::Black,
                Highlight::ExecLine => rgb(0, 130, 0),
            };
        }
    }
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
                Line::styled(text, if row == buffer.row { gutter_current() } else { gutter() })
            })
            .collect();
        frame.render_widget(Paragraph::new(numbers).style(gutter()), Rect::new(full.x, full.y, area.x - full.x, full.height));
    }
    let mut highlighter = LineHighlighter::new(buffer.lang());
    let width = area.width as usize;
    // The cursor is drawn as a colored block (the terminal's own cursor color
    // may be unreadable on the blue editor), so the real cursor stays hidden.
    let show_cursor = matches!(app.mode, Mode::Edit) && !app.output.visible;
    let breakpoints = buffer.path.as_ref().and_then(|p| app.breakpoints.get(p));
    let exec = app.exec_line();
    // Matches of the search bar, as (row, byte range).
    let mut matches: Vec<(usize, std::ops::Range<usize>)> = Vec::new();
    if let Some(bar) = &app.search
        && !bar.query.is_empty()
    {
        let mut starts = Vec::with_capacity(buffer.lines.len());
        let mut offset = 0;
        for line in &buffer.lines {
            starts.push(offset);
            offset += line.len() + 1;
        }
        for (from, to) in rust_ide::search::all(&buffer.text(), &bar.query, app.match_case) {
            let row = starts.partition_point(|&s| s <= from) - 1;
            matches.push((row, from - starts[row]..to - starts[row]));
        }
    }
    let selection = buffer.selection();
    let is_selected = |row: usize, byte: usize| selection.is_some_and(|(from, to)| (row, byte) >= from && (row, byte) < to);
    let mut lines = Vec::with_capacity(area.height as usize);
    // Lines above the view are still lexed, for multi-line comments/strings.
    for (row, text) in buffer.lines.iter().enumerate().take(buffer.scroll + area.height as usize) {
        let tokens = highlighter.line(text);
        if row < buffer.scroll {
            continue;
        }
        let line_style = if exec == Some(row) {
            Some(exec_line())
        } else if breakpoints.is_some_and(|b| b.contains(&row)) {
            Some(breakpoint_line())
        } else {
            None
        };
        let base = line_style.unwrap_or_else(editor);
        let mut styles = vec![base; text.len()];
        if line_style.is_none() {
            for (range, kind) in tokens {
                styles[range].fill(editor().fg(color(kind)));
            }
        }
        // The language server's errors and warnings are underlined.
        for d in app.live.iter().filter(|d| d.line <= row && row <= d.end_line) {
            let from = if d.line == row { byte_of(text, d.start) } else { 0 };
            let to = if d.end_line == row { byte_of(text, d.end).max(from + 1).min(text.len()) } else { text.len() };
            let kind = if d.error { Highlight::Error } else { Highlight::Warning };
            for style in &mut styles[from.min(text.len())..to] {
                *style = style.fg(color(kind)).add_modifier(Modifier::UNDERLINED);
            }
        }
        for (_, range) in matches.iter().filter(|(r, _)| *r == row) {
            styles[range.clone()].fill(match_style());
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
            let style = if show_cursor && row == buffer.row && char_index == buffer.char_col() {
                cursor()
            } else if is_selected(row, byte) {
                selected()
            } else {
                styles[byte]
            };
            spans.push(Span::styled(c.to_string(), style));
        }
        let len = text.chars().count();
        if show_cursor && row == buffer.row && buffer.char_col() >= len && buffer.char_col() < buffer.hscroll + width {
            spans.push(Span::styled(" ", cursor()));
        } else if is_selected(row, text.len()) && len >= buffer.hscroll && len < buffer.hscroll + width {
            // The line break of a selected line.
            spans.push(Span::styled(" ", selected()));
        }
        let mut line = Line::from(spans);
        if let Some(style) = line_style {
            // The bar goes across the whole line.
            line = line.style(style);
        }
        lines.push(line);
    }
    frame.render_widget(Paragraph::new(lines).style(editor()), area);
}

/// The byte index of a character column (clamped to the line).
fn byte_of(text: &str, chars: usize) -> usize {
    text.char_indices().nth(chars).map_or(text.len(), |(i, _)| i)
}

fn editor_style() -> Style {
    editor()
}

/// The Markdown preview of a buffer, scrolled down `scroll` lines.
fn draw_preview(frame: &mut Frame, source: &crate::buffer::Buffer, area: Rect, scroll: usize) {
    let base = editor();
    let styles = crate::markdown::Styles {
        text: base,
        heading: base.fg(color(Highlight::Keyword)).add_modifier(Modifier::BOLD),
        code: base.fg(color(Highlight::Str)),
        link: base.fg(color(Highlight::Type)).add_modifier(Modifier::UNDERLINED),
        quote: base.fg(color(Highlight::Comment)),
    };
    let block = Block::new().borders(ratatui::widgets::Borders::LEFT).border_style(base.fg(color(Highlight::Comment))).style(base);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = crate::markdown::render(&source.text(), inner.width.saturating_sub(1) as usize, styles);
    let scroll = scroll.min(lines.len().saturating_sub(1));
    let shown: Vec<Line> = lines.into_iter().skip(scroll).take(inner.height as usize).collect();
    frame.render_widget(Paragraph::new(shown).style(base), Rect { x: inner.x + 1, width: inner.width.saturating_sub(1), ..inner });
}

/// Legacy windows: Turbo Pascal's title line with the name and the window number.
fn draw_legacy_title(frame: &mut Frame, app: &App, area: Rect) {
    let star = if app.buffer().modified { "*" } else { "" };
    let name = format!("{}{star}", app.buffer().name());
    let left = format!("\u{2550}\u{2550}[\u{25A0}]\u{2550}\u{2550} {name} ");
    let right = format!(" {}/{} \u{2550}\u{2550}", app.active + 1, app.windows.len());
    let fill = (area.width as usize).saturating_sub(left.chars().count() + right.chars().count());
    let text = format!("{left}{}{right}", "\u{2550}".repeat(fill));
    let style = editor().fg(if theme() == Theme::Modern { Color::Black } else { Color::White });
    frame.render_widget(Paragraph::new(text).style(style), area);
}

/// Vertical tabs: the open files in a list left of the editor.
fn draw_window_list(frame: &mut Frame, app: &App, area: Rect) {
    let width = area.width as usize;
    let lines: Vec<Line> = app
        .windows
        .iter()
        .enumerate()
        .skip(list_first(app, area))
        .take(area.height as usize)
        .map(|(index, window)| {
            let star = if window.modified { "*" } else { "" };
            let text: String = format!(" {} {}{star}", index + 1, window.name()).chars().take(width).collect();
            let style = if index == app.active { chosen() } else { panel() };
            Line::styled(format!("{text:<width$}"), style)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(panel()), area);
}

/// The Find / Replace bar.
fn draw_search(frame: &mut Frame, app: &App, area: Rect) {
    let Some(search) = &app.search else { return };
    let field = |text: &str, active: bool| {
        let style = if active { input_style().add_modifier(Modifier::BOLD) } else { input_style() };
        let cursor = if active { "\u{2588}" } else { " " };
        Span::styled(format!(" {text}{cursor:<1}{}", " ".repeat(18usize.saturating_sub(text.chars().count()))), style)
    };
    let case = if app.match_case { "[x]" } else { "[ ]" };
    let mut spans = vec![Span::styled(" Find:", bar()), field(&search.query, search.field == 0)];
    if search.replace {
        spans.push(Span::styled("  Replace:", bar()));
        spans.push(field(&search.replacement, search.field == 1));
    }
    spans.push(Span::styled(format!("  {case} Match case (Alt+M)  "), bar()));
    let keys = if search.replace { "Enter replace  Alt+A all  Tab field  Esc" } else { "Enter/Down next  Up previous  Esc" };
    spans.push(Span::styled(keys, dim()));
    frame.render_widget(Paragraph::new(Line::from(spans)).style(bar()), area);
}

/// The Output window: the program's output and a line for its input.
fn draw_output(frame: &mut Frame, app: &App, area: Rect) {
    let pane = &app.output;
    let state = if pane.running { "running" } else { "finished" };
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Double)
        .title(format!(" Output - {} ({state}) ", pane.title))
        .style(Style::new().fg(rgb(200, 200, 200)).bg(Color::Black))
        .title_bottom(" Enter Send input   Ctrl+F2 Stop   Alt+F5/Esc Hide ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [text_area, input] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    let lines: Vec<&str> = pane.screen.lines().collect();
    let height = text_area.height as usize;
    let end = lines.len().saturating_sub(pane.scroll.min(lines.len().saturating_sub(height)));
    let start = end.saturating_sub(height);
    let shown: Vec<Line> = lines[start..end].iter().map(|l| Line::raw(l.replace('\t', "    "))).collect();
    frame.render_widget(Paragraph::new(shown), text_area);
    let prompt = if pane.running { format!("> {}\u{2588}", pane.input) } else { "(the program has ended)".into() };
    frame.render_widget(Paragraph::new(prompt).style(Style::new().fg(Color::White).bg(rgb(40, 40, 40))), input);
}

/// The debugger: watches, local variables and the call stack, side by side.
fn draw_debug(frame: &mut Frame, app: &App, area: Rect) {
    let [watches, locals, stack] =
        Layout::horizontal([Constraint::Percentage(34), Constraint::Percentage(33), Constraint::Percentage(33)]).areas(area);
    let heading = panel().fg(dark_blue()).add_modifier(Modifier::BOLD);
    let column = |title: &str, rows: Vec<String>, area: Rect| {
        let width = area.width as usize;
        let mut lines = vec![Line::styled(format!(" {title}"), heading)];
        if rows.is_empty() {
            lines.push(Line::styled(" (not running)", panel()));
        }
        lines.extend(rows.into_iter().take(area.height as usize).map(|r| Line::styled(r.chars().take(width).collect::<String>(), panel())));
        Paragraph::new(lines).style(panel())
    };
    let running = app.debug.is_some();
    let watch_rows: Vec<String> = app
        .watches
        .iter()
        .map(|(e, v)| if v.is_empty() || !running { format!(" {e}") } else { format!(" {e} = {v}") })
        .collect();
    let (local_rows, stack_rows) = match &app.debug {
        Some(debug) => (
            debug.locals.iter().map(|v| format!(" {} = {}", v.name, v.value)).collect(),
            debug.frames.iter().map(|f| format!(" {} line {}", f.name, f.line)).collect(),
        ),
        None => (Vec::new(), Vec::new()),
    };
    let mut watch_column = column("Watches  (Ctrl+F7 add)", watch_rows, watches);
    if app.watches.is_empty() {
        watch_column = column("Watches  (Ctrl+F7 add)", vec![" (none)".into()], watches);
    }
    frame.render_widget(watch_column, watches);
    frame.render_widget(column("Locals", local_rows, locals), locals);
    frame.render_widget(column("Call stack", stack_rows, stack), stack);
}

fn draw_messages(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![Line::styled(" Messages   Alt+F8 next  Alt+F7 previous", panel().fg(dark_blue()))];
    let first = app.message_index.unwrap_or(0).saturating_sub(area.height as usize - 2);
    let items = app.panel_items();
    for (index, m) in items.iter().enumerate().skip(first).take(area.height as usize - 1) {
        let place = match (&m.file, m.line) {
            (Some(file), line) if line > 0 => {
                format!("{}({line},{}): ", file.file_name().unwrap_or_default().to_string_lossy(), m.column)
            }
            _ => String::new(),
        };
        let style = if app.message_index == Some(index) {
            chosen()
        } else if m.level == "error" {
            panel().fg(dark_red())
        } else {
            panel()
        };
        lines.push(Line::styled(format!(" {} {place}{}", m.level.to_uppercase(), m.message), style));
    }
    frame.render_widget(Paragraph::new(lines).style(panel()), area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let buffer = app.buffer();
    let vim = app.vim.as_ref().map(|v| format!(" {}", v.status())).unwrap_or_default();
    let position = format!("{vim} {}:{} ", buffer.row + 1, buffer.char_col() + 1);
    let keys = " F1 Help  F2 Save  F3 Open  F5 Run  F9 Compile  F10 Menu ";
    let lsp: String = app.lsp_status.chars().take(30).collect();
    let toolchain = if lsp.is_empty() { app.toolchain_label() } else { format!("{lsp}  {}", app.toolchain_label()) };
    // The language server's message for the cursor's line comes first.
    let live = app.live.iter().find(|d| d.line <= buffer.row && buffer.row <= d.end_line);
    let status_text = match live {
        Some(d) => format!("{}: {}", if d.error { "Error" } else { "Warning" }, d.message.lines().next().unwrap_or("")),
        None => app.status.clone(),
    };
    let app_status = &status_text;
    if area.height >= 2 {
        // The message gets a line of its own above the keys.
        let room = (area.width as usize).saturating_sub(toolchain.chars().count() + position.chars().count() + 1);
        let status: String = app_status.chars().take(room).collect();
        let lines = vec![
            Line::from(vec![
                Span::styled(format!(" {status:<room$}"), bar().fg(dark_red())),
                Span::styled(toolchain, bar()),
                Span::styled(position, bar()),
            ]),
            Line::styled(keys, bar()),
        ];
        frame.render_widget(Paragraph::new(lines).style(bar()), area);
        return;
    }
    // Keep room for the message: the toolchain and server names give way first.
    let spare = (area.width as usize).saturating_sub(keys.len() + position.chars().count());
    let wanted = app_status.chars().count() + 1;
    let toolchain: String = toolchain.chars().take(spare.saturating_sub(wanted).max(spare.min(12))).collect();
    let room = (area.width as usize).saturating_sub(keys.len() + toolchain.chars().count() + position.chars().count());
    let status: String = app_status.chars().take(room).collect();
    let line = Line::from(vec![
        Span::styled(keys, bar()),
        Span::styled(format!("{status:<room$}"), bar().fg(dark_red())),
        Span::styled(toolchain, bar()),
        Span::styled(position, bar()),
    ]);
    frame.render_widget(Paragraph::new(line).style(bar()), area);
}

/// A Turbo-style double-bordered dialog box.
fn dialog(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Double)
        .title(format!(" {title} "))
        .style(Style::new().fg(Color::Black).bg(silver()))
        .border_style(Style::new().fg(Color::Black).bg(silver()))
        .title_style(Style::new().fg(Color::Black).bg(silver()).add_modifier(Modifier::BOLD))
}

fn centered(screen: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(screen.width);
    let height = height.min(screen.height);
    Rect::new(screen.x + (screen.width - width) / 2, screen.y + (screen.height - height) / 2, width, height)
}

fn draw_dropdown(frame: &mut Frame, app: &App, menu: Menu, selected: usize, bar: Rect) {
    let _ = bar;
    draw_menu_box(frame, &app.menu_items(menu), selected, dropdown_area(app, menu, frame.area()));
}

/// A framed list of menu items; separators are drawn as lines.
fn draw_menu_box(frame: &mut Frame, items: &[MenuItem], selected: usize, area: Rect) {
    frame.render_widget(Clear, area);
    let block = Block::bordered().style(bar()).border_style(bar());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            if item.is_separator() {
                return Line::styled("\u{2500}".repeat(inner.width as usize), bar());
            }
            let (normal, hot) = if index == selected { (chosen(), chosen_hot()) } else { (bar(), hot_style()) };
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
        PromptKind::GoTo => "Go to line number",
        PromptKind::Toolchain => "Rust toolchain of this project",
        PromptKind::Watch => "Add watch",
    };
    let area = centered(frame.area(), 60, 5);
    frame.render_widget(Clear, area);
    let block = dialog(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [field, hint] = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(inner);
    frame.render_widget(Paragraph::new(format!(" {input}")).style(input_style()), field);
    let hint_text = match kind {
        PromptKind::SaveAs => " Enter OK  Esc Cancel  (no extension: file type is added)",
        PromptKind::Toolchain => " Enter OK  Esc Cancel  (stable, nightly, 1.85.0 ...; empty = default)",
        _ => " Enter OK   Esc Cancel",
    };
    frame.render_widget(Paragraph::new(hint_text).style(bar()), hint);
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
    frame.render_widget(Paragraph::new(format!(" {}", dialog_state.dir.display())).style(bar()), dir);
    frame.render_widget(Paragraph::new(format!(" Name: {}", dialog_state.filter)).style(input_style()), field);
    let visible = dialog_state.visible();
    let first = dialog_state.selected.saturating_sub(list.height as usize - 1);
    let lines: Vec<Line> = visible
        .iter()
        .enumerate()
        .skip(first)
        .take(list.height as usize)
        .map(|(index, (name, is_dir))| {
            let label = if *is_dir { format!(" {name}\\") } else { format!(" {name}") };
            let style = if index == dialog_state.selected { chosen() } else { panel() };
            Line::styled(format!("{label:<width$}", width = list.width as usize), style)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(panel()), list);
    frame.render_widget(Paragraph::new(" Type to filter  Up/Down  Enter open  Backspace up  Esc").style(bar()), hint);
    frame.set_cursor_position(Position::new(field.x + 7 + dialog_state.filter.chars().count() as u16, field.y));
}

/// The shortcuts of the terminal IDE, shown by F1.
pub const SHORTCUTS: &[(&str, &str)] = &[
    ("F10 / Alt+letter", "Open a menu; arrows, Enter or the red letter choose"),
    ("F1", "This help (Up/Down/PgUp/PgDn scroll, Esc closes)"),
    ("F2 / Ctrl+S", "Save (asks for a name the first time)"),
    ("F3 / Ctrl+O", "Open a file (each in its own window)"),
    ("File > New", "Asks for the language: R Rust, P Python, C C#, T Text, M Markdown (or 1-5)"),
    ("File > New project", "Rust or Python, size Simple/Low/Medium/Complex, a name and a folder (Tab moves, Enter creates)"),
    ("Alt+X / Ctrl+Q", "Exit (asks to save or discard changes)"),
    ("Ctrl+Z / Ctrl+Y", "Undo / redo"),
    ("Shift+arrows", "Select text (also Shift+Home/End/PgUp/PgDn); typing replaces the selection"),
    ("Ctrl+X / Ctrl+C", "Cut / copy the selection (to the system clipboard)"),
    ("Ctrl+V", "Paste (many terminals paste with Ctrl+Shift+V or their own Ctrl+V)"),
    ("Ctrl+A", "Select all"),
    ("Ctrl+Space", "Word completion: words of this file and keywords (also opens by itself after two letters; Up/Down, Enter or Tab, Esc)"),
    ("Edit > Remove double empty lines", "Shrinks runs of empty lines to one, in the selected lines or the whole file"),
    ("Tab / Shift+Tab", "Indent / unindent"),
    ("Ctrl+Left/Right", "Previous / next word"),
    ("Ctrl+Home/End", "Start / end of the file"),
    ("Ctrl+F", "Find bar: Enter/Down next match, Up previous, Alt+M match case, Esc closes"),
    ("Ctrl+H / Ctrl+R", "Replace bar: Tab to the Replace field, Enter replaces one, Alt+A all (Ctrl+Z undoes)"),
    ("Ctrl+L", "Search again"),
    ("Alt+M", "Toggle match case"),
    ("Live errors", "The language server (rust-analyzer, basedpyright, csharp-ls) underlines errors as you type; the status line shows the one on the cursor's line; Alt+F8/F7 step through them"),
    ("'.' / '::'", "Completion from the language server; Ctrl+Space asks for it anywhere"),
    ("Options > Vim mode", "Vim keys: Normal, Insert, Visual, : commands (:w :q :wq), / search (remembered)"),
    ("Options > Settings", "The options in one list: check before run, line numbers, theme, two-line status, dependencies"),
    ("Options > Line numbers", "Show or hide the line numbers (remembered)"),
    ("Options > Two-line status", "Messages get a line of their own above the keys (remembered)"),
    ("Options > Theme", "Turbo Pascal colors, Mono (grays) or Modern (light) (remembered)"),
    ("Options > Check before run", "F9 / F5 look for errors first and stop if there are any (remembered)"),
    ("Tools > Dependencies", "Which tools are installed; I installs the chosen one, M shows the manual steps, C copies the command"),
    ("Tools > Rust toolchain", "Pick the Rust toolchain of this project from the installed ones (Up/Down, Enter, O other name); shown in the status line. On Windows use the -gnu ones"),
    ("Tools > Python .venv setup", "Create .venv in the project and install requirements.txt (or the project)"),
    ("Tools > General environment", "Edit proxy, PATH ... for everything the IDE starts (NAME=value file)"),
    ("Tools > Project environment", "Edit the variables of this project (.rust-ide/env; they override the general ones)"),
    ("Tools > Show environment", "List the variables that apply now"),
    ("Mouse", "Click a menu, a menu item, a tab or a place in the text; drag or Shift+click selects; right-click for Cut/Copy/Paste; the wheel moves the cursor (start with --no-mouse to leave the mouse to the terminal)"),
    ("Ctrl+G", "Go to line number"),
    ("F9", "Compile the current file"),
    ("Alt+F9", "Syntax check: find errors without building or running"),
    ("Alt+F8 / Alt+F7", "Next / previous error"),
    ("F5 / Ctrl+F9", "Save, compile and run; the output shows in the Output window (type input there)"),
    ("Run > Run in external terminal", "Run in a terminal window of its own (here when none can be opened)"),
    ("Alt+F5", "User screen: show or hide the Output window"),
    ("Ctrl+F2", "Program reset: stop the program or the debugger"),
    ("Ctrl+F8", "Toggle breakpoint on the cursor's line"),
    ("Shift+F5", "Debug run: stop at breakpoints (gdb 14+ for Rust, debugpy for Python, netcoredbg for C#)"),
    ("F7 / F8", "Trace into / step over (also start debugging); F5 continues"),
    ("Shift+F8", "Step out"),
    ("F4", "Go to cursor: debug and stop at the cursor's line"),
    ("Ctrl+F7", "Add watch (an expression shown at each stop)"),
    ("F6 / Shift+F6", "Next / previous window"),
    ("Alt+1 ... Alt+9", "Go to window 1-9; Alt+0 window list"),
    ("Ctrl+F3 / Alt+F3", "Close the window"),
    ("Alt+F6 / Alt+P", "Markdown preview of a .md file: side by side or in a tab (Window > Preview in)"),
    ("Window > styles", "Legacy windows (title line), horizontal tabs or vertical tabs (remembered)"),
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
        let (normal, hot) = if index == state.row { (chosen(), chosen_hot()) } else { (bar(), hot_style()) };
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
        let (normal, hot) = if index == state.row { (chosen(), chosen_hot()) } else { (bar(), hot_style()) };
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
    lines.push(Line::from(Span::styled(format!(" {problem}"), bar().fg(dark_red()))));
    lines.push(Line::from(" Tab/Up/Down field  Left/Right or letters change  Enter Create  Esc Cancel"));
    frame.render_widget(Paragraph::new(lines).style(bar()), inner);
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
            let (normal, hot) = if index == selected { (chosen(), chosen_hot()) } else { (bar(), hot_style()) };
            let mut spans = vec![Span::styled(" ", normal)];
            spans.extend(hotkey_spans(kind.label, kind.hotkey, normal, hot));
            let pad = (list.width as usize).saturating_sub(kind.label.chars().count() + kind.ext.len() + 3);
            spans.push(Span::styled(format!("{}.{} ", " ".repeat(pad), kind.ext), normal));
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(bar()), list);
    frame.render_widget(Paragraph::new(format!(" {}", filetype::NOTE)).style(dim()).wrap(ratatui::widgets::Wrap { trim: true }), note);
    let escape = if startup { "Esc: plain Rust window" } else { "Esc: cancel" };
    frame.render_widget(Paragraph::new(format!(" Up/Down  Enter OK  R P C T M / 1-5  {escape}")).style(bar()), hint);
}

fn draw_lessons(frame: &mut Frame, track: rust_ide::lessons::Track, selected: usize) {
    use crate::app::LESSON_ROWS;
    let lessons = track.lessons();
    let area = centered(frame.area(), 64, LESSON_ROWS as u16 + 4);
    frame.render_widget(Clear, area);
    let title = format!("Learn {}: lesson {} of {}", track.name(), selected + 1, lessons.len());
    let block = dialog(&title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [list, note, hint] =
        Layout::vertical([Constraint::Length(LESSON_ROWS as u16), Constraint::Length(1), Constraint::Length(1)]).areas(inner);
    // A window of rows that keeps the chosen lesson in view.
    let first = selected.saturating_sub(LESSON_ROWS / 2).min(lessons.len().saturating_sub(LESSON_ROWS));
    let lines: Vec<Line> = lessons
        .iter()
        .enumerate()
        .skip(first)
        .take(LESSON_ROWS)
        .map(|(index, lesson)| {
            let style = if index == selected { chosen() } else { bar() };
            let text = format!(" {:>2}  {}", lesson.number(), lesson.title());
            let pad = (list.width as usize).saturating_sub(text.chars().count());
            Line::from(Span::styled(format!("{text}{}", " ".repeat(pad)), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(bar()), list);
    frame.render_widget(Paragraph::new(" Run each lesson with F5. Your edits are kept.").style(dim()), note);
    frame.render_widget(Paragraph::new(" Up/Down PgUp/PgDn  Left/Right: language  Enter: open  Esc").style(bar()), hint);
}

fn draw_toolchains(frame: &mut Frame, choices: &[rust_ide::env::ToolchainChoice], selected: usize) {
    let height = (choices.len() as u16 + 1) * 2 + 5;
    let area = centered(frame.area(), 78, height);
    frame.render_widget(Clear, area);
    let block = dialog("Rust toolchain of this project");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines: Vec<Line> = Vec::new();
    let rows = choices.len() + 1;
    for index in 0..rows {
        let is_chosen = index == selected;
        let style = if is_chosen { chosen() } else { bar() };
        let (name, note, mark, usable) = match choices.get(index) {
            Some(c) => (if c.name.is_empty() { "Default" } else { c.name.as_str() }, c.note.as_str(), if c.current { "(*)" } else { "( )" }, c.usable),
            None => ("Other name...", "type a toolchain name yourself", "", true),
        };
        let pad = (inner.width as usize).saturating_sub(name.chars().count() + mark.len() + 2);
        lines.push(Line::from(Span::styled(format!(" {name}{}{mark} ", " ".repeat(pad)), style)));
        let note_style = if is_chosen { chosen() } else if usable { dim() } else { bar().fg(dark_red()) };
        lines.push(Line::from(Span::styled(format!("   {note}"), note_style)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(" Up/Down  Enter picks  O other name  Esc cancels"));
    lines.push(Line::from(" Install more with: rustup toolchain install <name>"));
    frame.render_widget(Paragraph::new(lines).style(bar()), inner);
}

fn draw_unsaved(frame: &mut Frame, app: &App, action: &Action, button: usize) {
    let area = centered(frame.area(), 52, 8);
    frame.render_widget(Clear, area);
    let block = dialog("Unsaved changes");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let unsaved = app.windows.iter().filter(|w| w.modified).count();
    let (what, ask) = match action {
        Action::Exit if unsaved > 1 => (format!("{unsaved} windows have unsaved changes. Save saves them all."), "Save before exiting?"),
        Action::Exit => ("A window has unsaved changes.".to_string(), "Save before exiting?"),
        _ => ("This window has unsaved changes.".to_string(), "Save before closing?"),
    };
    let mut buttons = vec![Span::raw(" ")];
    for (index, label) in ["Save", "Discard", "Cancel"].iter().enumerate() {
        buttons.push(Span::styled(format!(" {label} "), if index == button { chosen() } else { bar() }));
        buttons.push(Span::raw("  "));
    }
    let lines = vec![
        Line::from(format!(" {what}")),
        Line::from(format!(" {ask}")),
        Line::from(""),
        Line::from(buttons),
        Line::from(""),
        Line::from(" S save  D discard  C/Esc cancel  Tab/Enter choose"),
    ];
    frame.render_widget(Paragraph::new(lines).style(bar()), inner);
}

/// The word-completion list under the word being typed.
fn draw_completion(frame: &mut Frame, app: &App, full: Rect) {
    let Some(list) = &app.completion else { return };
    let items = app.visible_completions();
    if items.is_empty() {
        return;
    }
    let buffer = app.buffer();
    let area = text_area(app, full);
    let line = &buffer.lines[buffer.row];
    let start = line[..list.start.min(line.len())].chars().count();
    if buffer.row < buffer.scroll || start < buffer.hscroll {
        return;
    }
    let cursor_y = area.y + (buffer.row - buffer.scroll) as u16;
    let rows = items.len().min(8) as u16;
    let label_width = items.iter().map(|i| i.label.chars().count()).max().unwrap_or(1);
    let width = ((label_width + 11) as u16).min(area.width);
    let x = (area.x + (start - buffer.hscroll) as u16).min(area.right().saturating_sub(width));
    let y = if cursor_y + 1 + rows <= area.bottom() { cursor_y + 1 } else { cursor_y.saturating_sub(rows) };
    let popup = Rect::new(x, y, width, rows).intersection(frame.area());
    let first = list.selected.saturating_sub(rows as usize - 1);
    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .skip(first)
        .take(rows as usize)
        .map(|(index, item)| {
            let (style, note) = if index == list.selected { (chosen(), chosen()) } else { (bar(), dim()) };
            let kind: String = item.kind.chars().take(10).collect();
            let pad = (width as usize).saturating_sub(item.label.chars().count() + kind.len() + 2);
            Line::from(vec![
                Span::styled(format!(" {}{}", item.label, " ".repeat(pad)), style),
                Span::styled(format!("{kind} "), note),
            ])
        })
        .collect();
    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).style(bar()), popup);
}

/// Options > Settings.
fn draw_settings(frame: &mut Frame, app: &App, selected: usize) {
    let area = centered(frame.area(), 66, SETTINGS_ROWS.len() as u16 + 5);
    frame.render_widget(Clear, area);
    let block = dialog("Settings");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let on = |flag: bool| if flag { "[x]" } else { "[ ]" }.to_string();
    let values = [
        format!("< {} >", app.preview_mode.label()),
        format!("< {} >", app.window_style.label()),
        on(app.vim.is_some()),
        on(app.check_before),
        on(app.line_numbers),
        format!("< {} >", crate::app::theme_label()),
        on(app.two_line_status),
        String::new(),
        "Esc".to_string(),
    ];
    let width = inner.width as usize;
    let mut lines: Vec<Line> = SETTINGS_ROWS
        .iter()
        .zip(values)
        .enumerate()
        .map(|(index, ((label, key), value))| {
            let (normal, hot) = if index == selected { (chosen(), chosen_hot()) } else { (bar(), hot_style()) };
            let mut spans = vec![Span::styled(" ", normal)];
            spans.extend(hotkey_spans(label, *key, normal, hot));
            let pad = width.saturating_sub(label.chars().count() + value.chars().count() + 2);
            spans.push(Span::styled(format!("{}{value} ", " ".repeat(pad)), normal));
            Line::from(spans)
        })
        .collect();
    lines.push(Line::from(""));
    lines.push(Line::from(" Up/Down  Left/Right or Enter change  P W V C N T S D  Esc closes"));
    frame.render_widget(Paragraph::new(lines).style(bar()), inner);
}

/// Tools > Dependencies: one row per tool, details of the chosen one below.
fn draw_deps(frame: &mut Frame, state: &DepsDialog, check_at_startup: bool) {
    use rust_ide::deps::Need;
    let screen = frame.area();
    let height = (state.list.len().max(1) as u16 + 11).min(screen.height);
    let area = centered(screen, 100, height);
    frame.render_widget(Clear, area);
    let block = dialog("Dependencies");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [message, list, detail, hint] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(1), Constraint::Length(4), Constraint::Length(1)]).areas(inner);
    frame.render_widget(Paragraph::new(format!(" {}", state.message)).style(bar().fg(dark_red())), message);
    let width = list.width as usize;
    let first = state.selected.saturating_sub((list.height as usize).saturating_sub(1));
    let rows: Vec<Line> = state
        .list
        .iter()
        .enumerate()
        .skip(first)
        .take(list.height as usize)
        .map(|(index, dep)| {
            let need = match dep.need {
                Need::Required => "required",
                Need::Recommended => "recommended",
                Need::Optional => "optional",
            };
            let (mark, found) = match &dep.found {
                Some(found) => ("ok", found.clone()),
                None => ("--", "not found".to_string()),
            };
            let text = format!(" {mark} {:<24} {need:<12} {found}", dep.name);
            let text: String = text.chars().take(width).collect();
            let style = if index == state.selected {
                chosen()
            } else if dep.missing() && dep.need != Need::Optional {
                bar().fg(dark_red())
            } else {
                bar()
            };
            Line::styled(format!("{text:<width$}"), style)
        })
        .collect();
    frame.render_widget(Paragraph::new(rows).style(bar()), list);
    if let Some(dep) = state.list.get(state.selected) {
        let install = if dep.missing() {
            let auto = if dep.auto.is_some() { "  (I installs it)" } else { "" };
            format!(" Install: {}{auto}", dep.install)
        } else {
            String::new()
        };
        let lines = vec![Line::from(""), Line::styled(format!(" {}: {}", dep.name, dep.purpose), dim()), Line::from(install)];
        frame.render_widget(Paragraph::new(lines).style(bar()).wrap(ratatui::widgets::Wrap { trim: false }), detail);
    }
    let startup = if check_at_startup { "[x]" } else { "[ ]" };
    let keys = format!(" I install  C copy  M manual  R check again  G install guide  S {startup} check at startup  Esc");
    frame.render_widget(Paragraph::new(keys).style(bar()), hint);
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
                Span::styled(format!(" {keys:<20}"), panel().fg(dark_blue())),
                Span::styled(action.to_string(), panel()),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(panel()), inner);
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
        for expected in ["Choose a language", "Rust", "Python", "C#", "Text file", "Markdown", ".rs", ".cs", ".md", "Rust, Python and C# are supported"] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        assert!(text.contains("Esc: plain Rust window"));
        app.ask_language(false);
        let text = screen(&app, 80, 20);
        assert!(text.contains("New file: choose a language") && text.contains("Esc: cancel"));
    }

    #[test]
    fn draws_the_lessons_list() {
        let mut app = App::new(&[]);
        app.act(crate::app::Action::Learn(rust_ide::lessons::Track::Rust));
        let text = screen(&app, 80, 24);
        for expected in ["Learn Rust: lesson 1 of 25", "Hello world", "Ownership", "Enter: open"] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        app.mode = crate::app::Mode::Lessons { track: rust_ide::lessons::Track::CSharp, selected: 24 };
        let text = screen(&app, 80, 24);
        assert!(text.contains("Learn C#: lesson 25 of 25") && text.contains("Final project inventory"), "{text}");
        assert!(!text.contains("Hello world"), "the list scrolls to keep the lesson in view");
    }
    #[test]
    fn draws_the_new_project_dialog() {
        let _settings = crate::app::tests::isolate_settings();
        let mut app = App::new(&[]);
        app.ask_project();
        if let crate::app::Mode::Project(dialog) = &mut app.mode {
            dialog.lang = rust_ide::templates::Lang::Rust;
        }
        let text = screen(&app, 90, 24);
        for expected in ["New project", "Language", "< Rust >", "Size", "Name", "Folder", "Create", "Cancel", "Creates"] {
            assert!(text.contains(expected), "missing {expected:?} in
{text}");
        }
    }

    #[test]
    fn draws_the_unsaved_changes_dialog() {
        let mut app = App::new(&[]);
        app.mode = crate::app::Mode::Unsaved { action: Action::CloseWindow, button: 1 };
        let text = screen(&app, 90, 24);
        for expected in ["Unsaved changes", "Save before closing?", "Save", "Discard", "Cancel"] {
            assert!(text.contains(expected), "missing {expected:?} in {text}");
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
    fn draws_the_toolchain_picker() {
        let mut app = App::new(&[]);
        app.open_toolchains(&["stable-x86_64-pc-windows-gnu".to_string(), "nightly-x86_64-pc-windows-msvc".to_string()]);
        let text = screen(&app, 100, 30);
        for expected in ["Rust toolchain of this project", "Default", "stable-x86_64-pc-windows-gnu", "nightly-x86_64-pc-windows-msvc", "Other name...", "rustup toolchain install"] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
    }

    #[test]
    fn draws_separators_the_selection_and_the_right_click_menu() {
        let mut app = App::new(&[]);
        app.mode = Mode::Menu { menu: Menu::Edit, item: 0 };
        let text = screen(&app, 80, 16);
        assert!(text.contains("Remove double empty lines") && text.contains("Ctrl+X"), "{text}");
        assert!(text.contains("\u{2500}\u{2500}\u{2500}"), "separator lines: {text}");
        let mut app = App::new(&[]);
        app.buffer_mut().insert_text("one two");
        app.buffer_mut().go_to(0, 0);
        app.buffer_mut().start_selection();
        app.buffer_mut().word_right();
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal.draw(|frame| draw(frame, &app)).unwrap();
        let cells = terminal.backend().buffer().clone();
        assert_eq!(cells[(1, 1)].bg, selected().bg.unwrap(), "selected text");
        assert_eq!(cells[(6, 1)].bg, editor().bg.unwrap(), "after the selection");
        app.mode = Mode::Context { x: 35, y: 6, item: 0 };
        let text = screen(&app, 40, 8);
        assert!(text.contains("Cut") && text.contains("Select all"), "moved up and left to fit: {text}");
    }

    #[test]
    fn draws_the_completion_list_status_lines_and_dialogs() {
        let _settings = crate::app::tests::isolate_settings();
        let mut app = App::new(&[]);
        app.buffer_mut().ext = Some("rs");
        app.buffer_mut().insert_text("let counter = 1;\nco");
        app.complete(false);
        let text = screen(&app, 60, 12);
        assert!(text.contains(" counter") && text.contains("continue") && text.contains("keyword"), "{text}");
        app.completion = None;
        app.status = "Saved test.rs".into();
        app.two_line_status = true;
        let text = screen(&app, 60, 12);
        let rows: Vec<&str> = text.lines().collect();
        assert!(rows[10].starts_with(" Saved test.rs") && rows[11].contains("F1 Help"), "{text}");
        app.act(Action::Settings);
        let text = screen(&app, 80, 20);
        for expected in ["Settings", "Check before run", "Theme", "< Turbo >", "Two-line status", "[x]", "Dependencies..."] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        app.act(Action::Dependencies);
        let dep = rust_ide::deps::Dep {
            name: "gdb",
            purpose: "debugger",
            need: rust_ide::deps::Need::Recommended,
            found: None,
            install: "sudo apt install gdb".into(),
            auto: None,
        };
        app.deps_checked(vec![dep], false);
        let text = screen(&app, 110, 24);
        for expected in ["Dependencies", "gdb", "recommended", "not found", "sudo apt install gdb", "M manual", "G install guide"] {
            assert!(text.contains(expected), "missing {expected:?} in\n{text}");
        }
        // Every theme draws.
        for theme in ["mono", "modern", "turbo"] {
            crate::app::set_theme(theme);
            assert!(screen(&app, 110, 24).contains("Dependencies"));
        }
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
