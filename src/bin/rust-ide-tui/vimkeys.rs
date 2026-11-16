//! Vim mode for the terminal editor: runs the shared Vim engine (`rust_ide::vim`, written
//! for iced's text editor) on a copy of the buffer and copies the result back.

use iced::widget::text_editor::{Content, Cursor, Position};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rust_ide::vim::{self, Vim};

use crate::buffer::Buffer;

/// The Vim key for a terminal key, if Vim takes it in this mode. Insert mode only takes Esc,
/// so typing, arrows and the IDE's own keys work as usual (as in the window version).
pub fn binding(key: &KeyEvent, vim: &Vim) -> Option<vim::Key> {
    let named = match key.code {
        KeyCode::Esc => Some(vim::Key::Esc),
        KeyCode::Enter => Some(vim::Key::Enter),
        KeyCode::Backspace => Some(vim::Key::Backspace),
        KeyCode::Delete => Some(vim::Key::Delete),
        _ => None,
    };
    if vim.mode == vim::Mode::Insert && vim.command.is_none() {
        return named.filter(|k| *k == vim::Key::Esc);
    }
    if named.is_some() {
        return named;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char(c) if ctrl && !alt => Some(vim::Key::Ctrl(c.to_ascii_lowercase())),
        KeyCode::Char(c) if !alt => Some(vim::Key::Char(c)),
        _ => None,
    }
}

/// Runs one Vim key on the buffer. Text changes become one undo step.
pub fn key(vim: &mut Vim, key: vim::Key, buffer: &mut Buffer) -> Vec<vim::Effect> {
    let before = buffer.text();
    let mut content = Content::with_text(&before);
    let position = Position { line: buffer.row, column: buffer.col };
    match buffer.anchor {
        Some((line, column)) => content.move_to(Cursor { position, selection: Some(Position { line, column }) }),
        None => vim::place(&mut content, position),
    }
    let effects = vim.key(key, &mut content);
    let after = content.text();
    if after != before {
        buffer.set_text(&after);
    }
    let cursor = content.cursor();
    buffer.row = cursor.position.line.min(buffer.lines.len() - 1);
    buffer.col = cursor.position.column.min(buffer.lines[buffer.row].len());
    buffer.anchor = cursor.selection.filter(|s| *s != cursor.position).map(|s| (s.line, s.column));
    effects
}
