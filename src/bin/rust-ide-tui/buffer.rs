//! A text buffer for the terminal editor: lines, a cursor, a selection,
//! editing with auto-indent, and grouped undo/redo.
//!
//! The cursor column is a byte offset into the line, always on a character
//! boundary; the screen column is the number of characters before it.

use std::path::{Path, PathBuf};

use rust_ide::filetype;
use rust_ide::highlight::Lang;
use rust_ide::search;

const TAB: usize = 4;
const UNDO_LIMIT: usize = 200;

#[derive(Clone)]
struct Snapshot {
    lines: Vec<String>,
    row: usize,
    col: usize,
}

pub struct Buffer {
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
    pub path: Option<PathBuf>,
    /// Type chosen for a window that has no file name yet.
    pub ext: Option<&'static str>,
    pub modified: bool,
    /// First visible line and first visible character column.
    pub scroll: usize,
    pub hscroll: usize,
    /// Wanted character column for Up/Down through shorter lines.
    want: Option<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// Typing is undone in runs; moving the cursor starts a new run.
    group_open: bool,
    /// The other end of the selection (row, byte column); the cursor is one end.
    pub anchor: Option<(usize, usize)>,
    /// Counts changes to the text, so the language server can be told about them.
    pub version: u64,
    /// Never changes; lets a preview tab find its file when windows move.
    pub id: u64,
    /// A Markdown preview tab: the id of the window it shows (it has no text of its own).
    pub preview_of: Option<u64>,
    /// A name to show instead of the file's (preview tabs).
    pub title: Option<String>,
}

fn next_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Default for Buffer {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            row: 0,
            col: 0,
            path: None,
            ext: None,
            modified: false,
            scroll: 0,
            hscroll: 0,
            want: None,
            undo: Vec::new(),
            redo: Vec::new(),
            group_open: false,
            anchor: None,
            version: 0,
            id: next_id(),
            preview_of: None,
            title: None,
        }
    }
}

impl Buffer {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut buffer = Self::from_text(text.trim_start_matches('\u{feff}'));
        buffer.path = Some(path.to_path_buf());
        Ok(buffer)
    }

    /// A Markdown preview tab of the window with this id.
    pub fn preview(of: u64, title: String) -> Self {
        Self { preview_of: Some(of), title: Some(title), ..Self::default() }
    }

    /// An empty window of a chosen type, without a file name yet.
    pub fn untitled(ext: &'static str) -> Self {
        Self { ext: Some(ext), ..Self::default() }
    }

    pub fn from_text(text: &str) -> Self {
        let lines = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_string()).collect();
        Self { lines, ..Self::default() }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        let path = self.path.clone().ok_or_else(|| std::io::Error::other("no file name"))?;
        std::fs::write(path, self.text())?;
        self.modified = false;
        Ok(())
    }

    pub fn name(&self) -> String {
        if let Some(title) = &self.title {
            return title.clone();
        }
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .map_or(filetype::untitled_name(self.ext), |n| n.to_string_lossy().into_owned())
    }

    /// The language of the file: from its name, or from the chosen type.
    pub fn lang(&self) -> Lang {
        match &self.path {
            Some(path) => Lang::from_path(Some(path)),
            None => Lang::from_extension(self.ext),
        }
    }

    pub fn is_blank(&self) -> bool {
        self.path.is_none() && self.preview_of.is_none() && !self.modified && self.text().trim().is_empty()
    }

    fn line(&self) -> &str {
        &self.lines[self.row]
    }

    /// Characters before the cursor (the screen column).
    pub fn char_col(&self) -> usize {
        self.line()[..self.col].chars().count()
    }

    fn byte_at(line: &str, chars: usize) -> usize {
        line.char_indices().nth(chars).map_or(line.len(), |(i, _)| i)
    }

    // ----- Undo -----

    fn before_edit(&mut self) {
        if !self.group_open {
            self.undo.push(Snapshot { lines: self.lines.clone(), row: self.row, col: self.col });
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.group_open = true;
        }
        self.modified = true;
        self.want = None;
        self.anchor = None;
        self.version += 1;
    }

    /// Replaces all the text as one undo step (Vim commands), keeping the cursor where it was put.
    pub fn set_text(&mut self, text: &str) {
        self.end_group();
        self.before_edit();
        self.end_group();
        self.lines = text.split('\n').map(str::to_string).collect();
        self.row = self.row.min(self.lines.len() - 1);
        self.col = self.col.min(self.lines[self.row].len());
    }

    fn end_group(&mut self) {
        self.group_open = false;
    }

    pub fn undo(&mut self) -> bool {
        self.swap_snapshot(true)
    }

    pub fn redo(&mut self) -> bool {
        self.swap_snapshot(false)
    }

    fn swap_snapshot(&mut self, undo: bool) -> bool {
        let (from, to) = if undo { (&mut self.undo, &mut self.redo) } else { (&mut self.redo, &mut self.undo) };
        let Some(snapshot) = from.pop() else { return false };
        to.push(Snapshot { lines: std::mem::take(&mut self.lines), row: self.row, col: self.col });
        self.lines = snapshot.lines;
        self.row = snapshot.row.min(self.lines.len() - 1);
        self.col = snapshot.col.min(self.lines[self.row].len());
        self.modified = true;
        self.group_open = false;
        self.anchor = None;
        self.version += 1;
        true
    }

    // ----- Selection -----

    /// The selected range, start before end, or None when nothing is selected.
    pub fn selection(&self) -> Option<((usize, usize), (usize, usize))> {
        let anchor = self.anchor?;
        let cursor = (self.row, self.col);
        (anchor != cursor).then(|| if anchor < cursor { (anchor, cursor) } else { (cursor, anchor) })
    }

    /// Starts selecting at the cursor (Shift+arrows), unless a selection is already going.
    pub fn start_selection(&mut self) {
        if self.anchor.is_none() {
            self.anchor = Some((self.row, self.col));
        }
    }

    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    pub fn select_all(&mut self) {
        self.moved();
        self.anchor = Some((0, 0));
        self.row = self.lines.len() - 1;
        self.col = self.line().len();
    }

    pub fn selected_text(&self) -> Option<String> {
        let ((r1, c1), (r2, c2)) = self.selection()?;
        if r1 == r2 {
            return Some(self.lines[r1][c1..c2].to_string());
        }
        let mut text = self.lines[r1][c1..].to_string();
        for line in &self.lines[r1 + 1..r2] {
            text.push('\n');
            text.push_str(line);
        }
        text.push('\n');
        text.push_str(&self.lines[r2][..c2]);
        Some(text)
    }

    /// Deletes the selected text (one undo step with the typing that follows).
    /// Returns false when nothing was selected.
    pub fn delete_selection(&mut self) -> bool {
        let Some(((r1, c1), (r2, c2))) = self.selection() else {
            self.anchor = None;
            return false;
        };
        self.end_group();
        self.before_edit();
        let tail = self.lines[r2][c2..].to_string();
        self.lines[r1].truncate(c1);
        self.lines[r1].push_str(&tail);
        self.lines.drain(r1 + 1..=r2);
        self.row = r1;
        self.col = c1;
        true
    }

    /// Inserts text as it is (no auto-indent), replacing the selection; one undo step.
    pub fn insert_text(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        self.end_group();
        self.delete_selection();
        self.before_edit();
        let tail = self.lines[self.row].split_off(self.col);
        let mut parts = text.split('\n');
        self.lines[self.row].push_str(parts.next().unwrap_or(""));
        for part in parts {
            self.row += 1;
            self.lines.insert(self.row, part.to_string());
        }
        self.col = self.lines[self.row].len();
        self.lines[self.row].push_str(&tail);
        self.end_group();
    }

    /// Replaces the text from byte `start` of the cursor's line up to the cursor (completion).
    pub fn replace_before_cursor(&mut self, start: usize, with: &str) {
        let start = start.min(self.col);
        self.before_edit();
        let col = self.col;
        self.lines[self.row].replace_range(start..col, with);
        self.col = start + with.len();
        self.end_group();
    }

    /// Shrinks runs of empty lines to one, in the selected lines or the whole text, as one undo
    /// step. Returns how many lines went.
    pub fn remove_empty_lines(&mut self) -> usize {
        let range = self.selection().map(|((r1, _), (r2, _))| (r1, r2));
        let (text, removed) = rust_ide::text::remove_empty_lines(&self.text(), range);
        if removed == 0 {
            return 0;
        }
        let row = self.row;
        self.end_group();
        self.before_edit();
        self.lines = text.split('\n').map(str::to_string).collect();
        self.end_group();
        self.row = row.min(self.lines.len() - 1);
        self.col = self.col.min(self.line().len());
        while !self.lines[self.row].is_char_boundary(self.col) {
            self.col -= 1;
        }
        removed
    }

    // ----- Editing -----

    pub fn insert_char(&mut self, c: char) {
        self.before_edit();
        let col = self.col;
        self.lines[self.row].insert(col, c);
        self.col += c.len_utf8();
    }

    /// Splits the line, keeping the indentation (auto-indent).
    pub fn newline(&mut self) {
        self.before_edit();
        let rest = self.lines[self.row].split_off(self.col);
        let indent: String = self.lines[self.row].chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        self.row += 1;
        self.col = indent.len();
        self.lines.insert(self.row, indent + &rest);
        self.end_group();
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            self.before_edit();
            let prev = self.line()[..self.col].chars().next_back().map_or(0, char::len_utf8);
            self.col -= prev;
            let col = self.col;
            self.lines[self.row].remove(col);
        } else if self.row > 0 {
            self.before_edit();
            let line = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.lines[self.row].len();
            self.lines[self.row].push_str(&line);
        }
    }

    pub fn delete(&mut self) {
        if self.col < self.line().len() {
            self.before_edit();
            let col = self.col;
            self.lines[self.row].remove(col);
        } else if self.row + 1 < self.lines.len() {
            self.before_edit();
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
        }
    }

    /// Tab: spaces up to the next tab stop.
    pub fn indent(&mut self) {
        let spaces = TAB - self.char_col() % TAB;
        for _ in 0..spaces {
            self.insert_char(' ');
        }
    }

    /// Shift+Tab: removes up to one tab stop of leading spaces.
    pub fn unindent(&mut self) {
        let spaces = self.line().chars().take(TAB).take_while(|c| *c == ' ').count();
        if spaces > 0 {
            self.before_edit();
            self.lines[self.row].drain(..spaces);
            self.col = self.col.saturating_sub(spaces);
            self.end_group();
        }
    }

    // ----- Moving -----

    fn moved(&mut self) {
        self.end_group();
    }

    pub fn left(&mut self) {
        self.moved();
        self.want = None;
        if self.col > 0 {
            self.col -= self.line()[..self.col].chars().next_back().map_or(0, char::len_utf8);
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.line().len();
        }
    }

    pub fn right(&mut self) {
        self.moved();
        self.want = None;
        if self.col < self.line().len() {
            self.col += self.line()[self.col..].chars().next().map_or(0, char::len_utf8);
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    pub fn vertical(&mut self, delta: isize) {
        self.moved();
        let want = *self.want.get_or_insert(self.char_col());
        let last = self.lines.len() as isize - 1;
        self.row = (self.row as isize + delta).clamp(0, last) as usize;
        self.col = Self::byte_at(self.line(), want);
    }

    /// Home goes to the first non-blank character, then to column 0.
    pub fn home(&mut self) {
        self.moved();
        self.want = None;
        let first = self.line().len() - self.line().trim_start().len();
        self.col = if self.col == first { 0 } else { first };
    }

    pub fn end(&mut self) {
        self.moved();
        self.want = None;
        self.col = self.line().len();
    }

    pub fn word_right(&mut self) {
        self.moved();
        self.want = None;
        let line = self.line();
        if self.col >= line.len() {
            return self.right();
        }
        let rest = &line[self.col..];
        let word = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
        let after = &rest[word..];
        let space = after.find(|c: char| !c.is_whitespace()).unwrap_or(after.len());
        self.col += (word + space).max(rest.chars().next().map_or(0, char::len_utf8));
    }

    pub fn word_left(&mut self) {
        self.moved();
        self.want = None;
        if self.col == 0 {
            return self.left();
        }
        let before = &self.line()[..self.col];
        let trimmed = before.trim_end();
        let start = trimmed
            .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
            .map_or(0, |i| i + trimmed[i..].chars().next().map_or(1, char::len_utf8));
        self.col = if start < self.col { start } else { trimmed.len() - trimmed.chars().next_back().map_or(0, char::len_utf8) };
    }

    pub fn document_start(&mut self) {
        self.moved();
        self.row = 0;
        self.col = 0;
    }

    pub fn document_end(&mut self) {
        self.moved();
        self.row = self.lines.len() - 1;
        self.col = self.line().len();
    }

    /// Moves to a 0-based line and character column (clamped), dropping the selection.
    pub fn go_to(&mut self, row: usize, char_col: usize) {
        self.anchor = None;
        self.move_to(row, char_col);
    }

    /// Like `go_to`, but keeps the selection (dragging with the mouse).
    pub fn move_to(&mut self, row: usize, char_col: usize) {
        self.moved();
        self.want = None;
        self.row = row.min(self.lines.len() - 1);
        self.col = Self::byte_at(self.line(), char_col);
    }

    /// Replaces every match of `query` (`case`: match case) as one undoable step;
    /// returns how many were replaced.
    pub fn replace_all(&mut self, query: &str, with: &str, case: bool) -> usize {
        let text = self.text();
        let (new_text, count) = search::replace_all(&text, query, with, case);
        if count == 0 {
            return 0;
        }
        let offset = self.offset().min(new_text.len());
        self.end_group();
        self.before_edit();
        self.lines = new_text.split('\n').map(str::to_string).collect();
        self.end_group();
        self.go_to_offset(offset);
        // The offset may fall inside a character after the text changed.
        while !self.lines[self.row].is_char_boundary(self.col) {
            self.col -= 1;
        }
        count
    }

    /// Byte offset of the cursor in `text()`.
    pub fn offset(&self) -> usize {
        self.offset_of(self.row, self.col)
    }

    /// Byte offset of a (row, byte column) in `text()`.
    pub fn offset_of(&self, row: usize, col: usize) -> usize {
        self.lines[..row].iter().map(|l| l.len() + 1).sum::<usize>() + col
    }

    /// Replaces the selected text (Replace one); one undo step.
    pub fn replace_selection(&mut self, with: &str) {
        self.insert_text(with);
    }

    /// Moves to a byte offset in `text()`.
    pub fn go_to_offset(&mut self, offset: usize) {
        self.anchor = None;
        self.go_to_offset_keeping(offset);
    }

    /// Moves to a byte offset, keeping the selection (so it ends there).
    pub fn go_to_offset_keeping(&mut self, offset: usize) {
        self.moved();
        self.want = None;
        let mut rest = offset;
        for (row, line) in self.lines.iter().enumerate() {
            if rest <= line.len() {
                self.row = row;
                self.col = rest;
                return;
            }
            rest -= line.len() + 1;
        }
        self.document_end();
    }

    /// Keeps the cursor inside a `height` x `width` view.
    pub fn scroll_into_view(&mut self, height: usize, width: usize) {
        if height > 0 {
            if self.row < self.scroll {
                self.scroll = self.row;
            } else if self.row >= self.scroll + height {
                self.scroll = self.row + 1 - height;
            }
        }
        let col = self.char_col();
        if width > 0 {
            if col < self.hscroll {
                self.hscroll = col;
            } else if col >= self.hscroll + width {
                self.hscroll = col + 1 - width;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> Buffer {
        let mut b = Buffer::default();
        for c in text.chars() {
            if c == '\n' { b.newline() } else { b.insert_char(c) }
        }
        b
    }

    #[test]
    fn word_left_stays_on_character_boundaries() {
        for text in ["x = ไม่", "a →", "“q“", "ok ✅", "ไม่ใช่ ไทย"] {
            let mut b = typed(text);
            let mut steps = 0;
            while b.col > 0 {
                b.word_left();
                assert!(b.line().is_char_boundary(b.col), "{text:?} col {}", b.col);
                let _ = b.char_col();
                steps += 1;
                assert!(steps < 50, "no progress in {text:?}");
            }
            for _ in 0..20 {
                b.word_right();
                assert!(b.line().is_char_boundary(b.col), "{text:?} col {}", b.col);
            }
        }
    }

    #[test]
    fn typing_with_auto_indent() {
        let b = typed("fn main() {\n    let x = 1;\nx");
        assert_eq!(b.text(), "fn main() {\n    let x = 1;\n    x");
        assert!(b.modified);
    }

    #[test]
    fn backspace_and_delete_join_lines() {
        let mut b = Buffer::from_text("ab\ncd");
        b.go_to(1, 0);
        b.backspace();
        assert_eq!(b.text(), "abcd");
        assert_eq!((b.row, b.col), (0, 2));
        b.end();
        b.delete();
        assert_eq!(b.text(), "abcd", "delete at the very end does nothing");
    }

    #[test]
    fn thai_text_moves_by_characters() {
        let mut b = Buffer::from_text("ไทย ok");
        b.right();
        b.right();
        assert_eq!(b.char_col(), 2);
        b.backspace();
        assert_eq!(b.text(), "ไย ok");
        b.end();
        b.vertical(0);
        assert_eq!(b.char_col(), 5);
    }

    #[test]
    fn up_down_remember_the_column() {
        let mut b = Buffer::from_text("long line here\nab\nanother long line");
        b.go_to(0, 10);
        b.vertical(1);
        assert_eq!(b.char_col(), 2, "clamped on the short line");
        b.vertical(1);
        assert_eq!(b.char_col(), 10, "back to the wanted column");
    }

    #[test]
    fn undo_groups_typing_runs() {
        let mut b = typed("abc");
        b.left();
        b.insert_char('X');
        assert_eq!(b.text(), "abXc");
        assert!(b.undo());
        assert_eq!(b.text(), "abc");
        assert!(b.undo());
        assert_eq!(b.text(), "");
        assert!(b.redo());
        assert_eq!(b.text(), "abc");
    }

    #[test]
    fn tab_indents_to_the_next_stop() {
        let mut b = typed("ab");
        b.indent();
        assert_eq!(b.text(), "ab  ");
        b.home();
        b.home();
        b.indent();
        assert_eq!(b.text(), "    ab  ");
        b.unindent();
        assert_eq!(b.text(), "ab  ");
    }

    #[test]
    fn words_and_offsets() {
        let mut b = Buffer::from_text("let value = 42;\nnext");
        b.word_right();
        assert_eq!(b.char_col(), 4);
        b.word_right();
        assert_eq!(b.char_col(), 10);
        b.word_left();
        assert_eq!(b.char_col(), 4);
        b.go_to_offset(17);
        assert_eq!((b.row, b.char_col()), (1, 1));
        assert_eq!(b.offset(), 17);
    }

    #[test]
    fn selections_copy_delete_and_paste() {
        let mut b = Buffer::from_text("one two\nthree\nfour");
        b.go_to(0, 4);
        b.start_selection();
        b.vertical(1);
        b.end();
        assert_eq!(b.selected_text().as_deref(), Some("two\nthree"));
        assert!(b.delete_selection());
        assert_eq!(b.text(), "one \nfour");
        assert_eq!((b.row, b.col), (0, 4));
        assert!(b.selection().is_none());
        b.insert_text("a\r\nb");
        assert_eq!(b.text(), "one a\nb\nfour");
        assert_eq!((b.row, b.col), (1, 1));
        b.undo();
        assert_eq!(b.text(), "one \nfour", "a paste is one undo step");
        b.select_all();
        assert_eq!(b.selected_text().as_deref(), Some("one \nfour"));
        b.delete_selection();
        b.insert_char('x');
        assert_eq!(b.text(), "x");
        b.undo();
        assert_eq!(b.text(), "one \nfour", "deleting and typing undo together");
    }

    #[test]
    fn selection_ends_backwards_and_thai() {
        let mut b = Buffer::from_text("ไทย ok");
        b.end();
        b.start_selection();
        b.left();
        b.left();
        b.left();
        assert_eq!(b.selected_text().as_deref(), Some(" ok"));
        b.home();
        assert_eq!(b.selected_text().as_deref(), Some("ไทย ok"));
        b.go_to(0, 0);
        assert!(b.selection().is_none(), "go_to drops it");
    }

    #[test]
    fn double_empty_lines_shrink_in_one_step() {
        let mut b = Buffer::from_text("a\n\n\n\nb\n\nc");
        assert_eq!(b.remove_empty_lines(), 2);
        assert_eq!(b.text(), "a\n\nb\n\nc");
        b.undo();
        assert_eq!(b.text(), "a\n\n\n\nb\n\nc");
        assert_eq!(Buffer::from_text("a\n\nb").remove_empty_lines(), 0);
    }

    #[test]
    fn completion_replaces_the_word_start() {
        let mut b = Buffer::from_text("let va");
        b.end();
        b.replace_before_cursor(4, "value");
        assert_eq!(b.text(), "let value");
        assert_eq!(b.char_col(), 9);
    }

    #[test]
    fn keeps_cursor_in_view() {
        let mut b = Buffer::from_text(&"x\n".repeat(100));
        b.go_to(50, 0);
        b.scroll_into_view(20, 80);
        assert_eq!(b.scroll, 31);
        b.go_to(5, 0);
        b.scroll_into_view(20, 80);
        assert_eq!(b.scroll, 5);
    }
}
