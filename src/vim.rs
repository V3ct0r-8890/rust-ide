//! A basic Vim emulation on top of iced's text editor: Normal, Insert and
//! Visual modes, counts, d/c/y/>/< operators with motions, registers,
//! `/` search and a few `:` commands.
//!
//! Editor columns are byte offsets into the line (as in iced/cosmic-text);
//! helpers below keep every offset on a character boundary.

use std::sync::Arc;

use iced::widget::text_editor::{Action, Content, Cursor, Edit, Motion, Position};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Visual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Esc,
    Enter,
    Backspace,
    Delete,
    Ctrl(char),
}

/// What the IDE must do after a key, beyond the buffer changes Vim made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// The buffer changed (the IDE records undo, marks the file modified).
    Edited,
    Save,
    Quit,
    ForceQuit,
    SaveQuit,
    Undo,
    Redo,
    Status(String),
}

pub struct Vim {
    pub mode: Mode,
    /// Count and operator typed so far, e.g. "2d" or "g".
    pending: String,
    register: String,
    linewise: bool,
    /// The `:` or `/` command line being typed, including its first char.
    pub command: Option<String>,
    search: String,
    /// Where Visual mode started.
    anchor: Position,
}

/// A motion and how far it goes.
enum Target {
    /// Repeat an editor motion, character-wise.
    Act(Motion, usize),
    /// First non-blank character of the current line.
    FirstNonBlank,
    /// `n` lines down (positive) or up (negative), line-wise.
    Lines(isize),
    /// An absolute line, line-wise.
    Line(usize),
}

impl Default for Vim {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            pending: String::new(),
            register: String::new(),
            linewise: false,
            command: None,
            search: String::new(),
            anchor: Position { line: 0, column: 0 },
        }
    }
}

impl Vim {
    /// Short text for the status bar.
    pub fn status(&self) -> String {
        if let Some(command) = &self.command {
            return command.clone();
        }
        let mode = match self.mode {
            Mode::Normal => "NORMAL",
            Mode::Insert => "-- INSERT --",
            Mode::Visual => "-- VISUAL --",
        };
        if self.pending.is_empty() { mode.into() } else { format!("{mode} {}", self.pending) }
    }

    pub fn key(&mut self, key: Key, c: &mut Content) -> Vec<Effect> {
        if self.command.is_some() {
            return self.command_key(key, c);
        }
        match self.mode {
            Mode::Insert => {
                if key == Key::Esc {
                    self.mode = Mode::Normal;
                    if c.cursor().position.column > 0 {
                        c.perform(Action::Move(Motion::Left));
                    }
                }
                Vec::new()
            }
            Mode::Normal => self.normal(key, c),
            Mode::Visual => self.visual(key, c),
        }
    }

    fn normal(&mut self, key: Key, c: &mut Content) -> Vec<Effect> {
        let ch = match key {
            Key::Esc => {
                self.pending.clear();
                return Vec::new();
            }
            Key::Enter => 'j',
            Key::Backspace => 'h',
            Key::Delete => 'x',
            Key::Ctrl('r') => return vec![Effect::Redo],
            Key::Ctrl('d') => return move_n(c, Motion::PageDown, 1),
            Key::Ctrl('u') => return move_n(c, Motion::PageUp, 1),
            Key::Ctrl(_) => return Vec::new(),
            Key::Char(ch) => ch,
        };

        if self.pending.ends_with('r') && !self.pending.ends_with("rr") {
            let (count, _) = self.take_pending();
            return replace_chars(c, ch, count);
        }
        let digits = self.pending.chars().all(|d| d.is_ascii_digit());
        if ch.is_ascii_digit() && digits && (ch != '0' || !self.pending.is_empty()) {
            self.pending.push(ch);
            return Vec::new();
        }

        let operator = self.pending.trim_start_matches(|d: char| d.is_ascii_digit()).to_string();
        let g = operator.ends_with('g');
        let op = operator.trim_end_matches('g').chars().next();

        // Keys that only extend the pending command.
        if ch == 'g' && !g {
            self.pending.push('g');
            return Vec::new();
        }
        if op.is_none() && !g && matches!(ch, 'd' | 'c' | 'y' | '>' | '<' | 'r') {
            self.pending.push(ch);
            return Vec::new();
        }

        let (count, has_count) = self.take_pending();
        let line = c.cursor().position.line;

        // Doubled operator: dd, cc, yy, >>, <<
        if let Some(op) = op
            && ch == op
        {
            return self.lines_op(op, line, line + count - 1, c);
        }

        if let Some(target) = target(ch, g, count, has_count, c) {
            return match op {
                None => go(c, target),
                Some(op) => self.operate(op, target, c),
            };
        }
        if op.is_some() || g {
            return Vec::new(); // unknown motion after an operator: cancel
        }

        match ch {
            'i' => self.insert(),
            'a' => {
                if col(c) < line_len(c, line) {
                    c.perform(Action::Move(Motion::Right));
                }
                self.insert()
            }
            'I' => {
                go(c, Target::FirstNonBlank);
                self.insert()
            }
            'A' => {
                c.perform(Action::Move(Motion::End));
                self.insert()
            }
            'o' => {
                c.perform(Action::Move(Motion::End));
                c.perform(Action::Edit(Edit::Enter));
                self.mode = Mode::Insert;
                vec![Effect::Edited]
            }
            'O' => {
                c.perform(Action::Move(Motion::Home));
                c.perform(Action::Edit(Edit::Enter));
                c.perform(Action::Move(Motion::Up));
                self.mode = Mode::Insert;
                vec![Effect::Edited]
            }
            'x' => self.delete_chars(c, count, true),
            'X' => self.delete_chars(c, count, false),
            's' => {
                let mut effects = self.delete_chars(c, count, true);
                self.mode = Mode::Insert;
                effects.push(Effect::Edited);
                effects
            }
            'S' => self.lines_op('c', line, line + count - 1, c),
            'D' => self.operate('d', Target::Act(Motion::End, 1), c),
            'C' => self.operate('c', Target::Act(Motion::End, 1), c),
            'Y' => self.lines_op('y', line, line + count - 1, c),
            'p' => self.paste(c, count, true),
            'P' => self.paste(c, count, false),
            'J' => join_lines(c, count.max(2) - 1),
            'u' => vec![Effect::Undo],
            'v' => {
                self.mode = Mode::Visual;
                self.anchor = c.cursor().position;
                Vec::new()
            }
            'n' => self.find_next(c, true),
            'N' => self.find_next(c, false),
            ':' | '/' => {
                self.command = Some(ch.to_string());
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn visual(&mut self, key: Key, c: &mut Content) -> Vec<Effect> {
        let ch = match key {
            Key::Esc => {
                self.leave_visual(c);
                return Vec::new();
            }
            Key::Char(ch) => ch,
            Key::Enter => 'j',
            Key::Backspace => 'h',
            Key::Delete => 'x',
            Key::Ctrl(_) => return Vec::new(),
        };
        let digits = self.pending.chars().all(|d| d.is_ascii_digit());
        if ch.is_ascii_digit() && digits && (ch != '0' || !self.pending.is_empty()) {
            self.pending.push(ch);
            return Vec::new();
        }
        let g = self.pending.ends_with('g');
        if ch == 'g' && !g {
            self.pending.push('g');
            return Vec::new();
        }
        let (count, has_count) = self.take_pending();

        if let Some(target) = target(ch, g, count, has_count, c) {
            // Move the free end from the cursor (not from the selection edge).
            let position = c.cursor().position;
            place(c, position);
            go(c, target);
            let position = c.cursor().position;
            c.move_to(Cursor { position, selection: Some(self.anchor) });
            return Vec::new();
        }

        let (from, to) = self.visual_range(c);
        match ch {
            'd' | 'x' | 'c' | 's' | 'y' => {
                select(c, from, to);
                self.register = c.selection().unwrap_or_default();
                self.linewise = false;
                if ch == 'y' {
                    place(c, from);
                    self.mode = Mode::Normal;
                    return vec![Effect::Status("yanked".into())];
                }
                c.perform(Action::Edit(Edit::Delete));
                self.mode = if matches!(ch, 'c' | 's') { Mode::Insert } else { Mode::Normal };
                vec![Effect::Edited]
            }
            '>' | '<' => {
                let effects = indent(c, from.line, to.line, ch == '>');
                self.mode = Mode::Normal;
                effects
            }
            'v' => {
                self.leave_visual(c);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn leave_visual(&mut self, c: &mut Content) {
        let position = c.cursor().position;
        place(c, position);
        self.mode = Mode::Normal;
        self.pending.clear();
    }

    /// The selected range, inclusive of the character under the far end.
    fn visual_range(&self, c: &Content) -> (Position, Position) {
        let cursor = c.cursor().position;
        let (from, to) = ordered(self.anchor, cursor);
        let end = next_char(c, to);
        (from, end)
    }

    fn command_key(&mut self, key: Key, c: &mut Content) -> Vec<Effect> {
        let Some(command) = &mut self.command else { return Vec::new() };
        match key {
            Key::Esc => self.command = None,
            Key::Backspace => {
                command.pop();
                if command.is_empty() {
                    self.command = None;
                }
            }
            Key::Char(ch) => command.push(ch),
            Key::Enter => {
                let command = self.command.take().unwrap_or_default();
                return self.run_command(&command, c);
            }
            Key::Delete | Key::Ctrl(_) => {}
        }
        Vec::new()
    }

    fn run_command(&mut self, command: &str, c: &mut Content) -> Vec<Effect> {
        if let Some(pattern) = command.strip_prefix('/') {
            if !pattern.is_empty() {
                self.search = pattern.to_string();
            }
            return self.find_next(c, true);
        }
        let command = command.trim_start_matches(':').trim();
        match command {
            "w" => vec![Effect::Save],
            "q" => vec![Effect::Quit],
            "q!" => vec![Effect::ForceQuit],
            "wq" | "x" => vec![Effect::SaveQuit],
            "" => Vec::new(),
            _ => match command.parse::<usize>() {
                Ok(line) => go(c, Target::Line(line.saturating_sub(1))),
                Err(_) => vec![Effect::Status(format!("Not an editor command: {command}"))],
            },
        }
    }

    fn insert(&mut self) -> Vec<Effect> {
        self.mode = Mode::Insert;
        Vec::new()
    }

    /// Returns (count, whether a count was typed) and clears the pending keys.
    fn take_pending(&mut self) -> (usize, bool) {
        let digits: String = self.pending.chars().take_while(char::is_ascii_digit).collect();
        self.pending.clear();
        match digits.parse::<usize>() {
            Ok(n) if n > 0 => (n, true),
            _ => (1, false),
        }
    }

    /// Applies operator `op` over a motion.
    fn operate(&mut self, op: char, target: Target, c: &mut Content) -> Vec<Effect> {
        let start = c.cursor().position;
        match target {
            Target::Lines(n) => {
                let other = (start.line as isize + n).max(0) as usize;
                let (a, b) = (start.line.min(other), start.line.max(other));
                self.lines_op(op, a, b, c)
            }
            Target::Line(line) => {
                let (a, b) = (start.line.min(line), start.line.max(line));
                self.lines_op(op, a, b, c)
            }
            target => {
                if matches!(op, '>' | '<') {
                    return indent(c, start.line, start.line, op == '>');
                }
                go(c, target);
                let end = c.cursor().position;
                let (from, to) = ordered(start, end);
                select(c, from, to);
                self.register = c.selection().unwrap_or_default();
                self.linewise = false;
                match op {
                    'y' => {
                        place(c, from);
                        Vec::new()
                    }
                    _ => {
                        if from != to {
                            c.perform(Action::Edit(Edit::Delete));
                        } else {
                            place(c, from);
                        }
                        if op == 'c' {
                            self.mode = Mode::Insert;
                        }
                        vec![Effect::Edited]
                    }
                }
            }
        }
    }

    /// Line-wise operator over lines `first..=last`.
    fn lines_op(&mut self, op: char, first: usize, last: usize, c: &mut Content) -> Vec<Effect> {
        let last = last.min(c.line_count().saturating_sub(1));
        if matches!(op, '>' | '<') {
            return indent(c, first, last, op == '>');
        }
        self.register = (first..=last).map(|l| line_text(c, l) + "\n").collect();
        self.linewise = true;
        let lines = last - first + 1;
        match op {
            'y' => vec![Effect::Status(format!("{lines} line(s) yanked"))],
            'c' => {
                let indent: String = line_text(c, first).chars().take_while(|ch| ch.is_whitespace()).collect();
                select(c, Position { line: first, column: 0 }, end_of(c, last));
                c.perform(Action::Edit(Edit::Paste(Arc::new(indent))));
                self.mode = Mode::Insert;
                vec![Effect::Edited]
            }
            _ => {
                delete_lines(c, first, last);
                let line = first.min(c.line_count().saturating_sub(1));
                go(c, Target::Line(line));
                vec![Effect::Edited]
            }
        }
    }

    fn delete_chars(&mut self, c: &mut Content, count: usize, forward: bool) -> Vec<Effect> {
        let at = c.cursor().position;
        let text = line_text(c, at.line);
        let (from, to) = if forward {
            (at.column, advance(&text, at.column, count))
        } else {
            (retreat(&text, at.column, count), at.column)
        };
        if from == to {
            return Vec::new();
        }
        self.register = text[from..to].to_string();
        self.linewise = false;
        select(c, Position { line: at.line, column: from }, Position { line: at.line, column: to });
        c.perform(Action::Edit(Edit::Delete));
        vec![Effect::Edited]
    }

    fn paste(&mut self, c: &mut Content, count: usize, after: bool) -> Vec<Effect> {
        if self.register.is_empty() {
            return Vec::new();
        }
        let text = self.register.repeat(count);
        let line = c.cursor().position.line;
        if self.linewise {
            if after {
                let body = text.strip_suffix('\n').unwrap_or(&text);
                place(c, end_of(c, line));
                c.perform(Action::Edit(Edit::Paste(Arc::new(format!("\n{body}")))));
                go(c, Target::Line(line + 1));
            } else {
                place(c, Position { line, column: 0 });
                c.perform(Action::Edit(Edit::Paste(Arc::new(text))));
                go(c, Target::Line(line));
            }
        } else {
            if after && col(c) < line_len(c, line) {
                c.perform(Action::Move(Motion::Right));
            }
            c.perform(Action::Edit(Edit::Paste(Arc::new(text))));
        }
        vec![Effect::Edited]
    }

    fn find_next(&mut self, c: &mut Content, forward: bool) -> Vec<Effect> {
        if self.search.is_empty() {
            return Vec::new();
        }
        let text = c.text();
        let here = offset_of(&text, c.cursor().position);
        let found = if forward {
            let start = (here + 1).min(text.len());
            let start = (start..=text.len()).find(|&i| text.is_char_boundary(i)).unwrap_or(text.len());
            text[start..].find(&self.search).map(|i| start + i).or_else(|| text.find(&self.search))
        } else {
            text[..here].rfind(&self.search).or_else(|| text.rfind(&self.search))
        };
        match found {
            Some(offset) => {
                place(c, position_of(&text, offset));
                Vec::new()
            }
            None => vec![Effect::Status(format!("Pattern not found: {}", self.search))],
        }
    }
}

/// Resolves a motion key.
fn target(ch: char, g: bool, count: usize, has_count: bool, c: &Content) -> Option<Target> {
    Some(match (g, ch) {
        (true, 'g') => Target::Line(if has_count { count - 1 } else { 0 }),
        (true, _) => return None,
        (_, 'h') => Target::Act(Motion::Left, count),
        (_, 'l' | ' ') => Target::Act(Motion::Right, count),
        (_, 'w' | 'W' | 'e' | 'E') => Target::Act(Motion::WordRight, count),
        (_, 'b' | 'B') => Target::Act(Motion::WordLeft, count),
        (_, '0') => Target::Act(Motion::Home, 1),
        (_, '$') => Target::Act(Motion::End, 1),
        (_, '^') => Target::FirstNonBlank,
        (_, 'j') => Target::Lines(count as isize),
        (_, 'k') => Target::Lines(-(count as isize)),
        (_, 'G') => Target::Line(if has_count { count - 1 } else { c.line_count().saturating_sub(1) }),
        _ => return None,
    })
}

/// Moves the cursor to a target.
fn go(c: &mut Content, target: Target) -> Vec<Effect> {
    match target {
        Target::Act(motion, n) => return move_n(c, motion, n),
        Target::FirstNonBlank => {
            let line = c.cursor().position.line;
            let column = first_non_blank(&line_text(c, line));
            place(c, Position { line, column });
        }
        Target::Lines(n) => {
            let motion = if n > 0 { Motion::Down } else { Motion::Up };
            return move_n(c, motion, n.unsigned_abs());
        }
        Target::Line(line) => {
            let line = line.min(c.line_count().saturating_sub(1));
            let column = first_non_blank(&line_text(c, line));
            place(c, Position { line, column });
        }
    }
    Vec::new()
}

fn move_n(c: &mut Content, motion: Motion, n: usize) -> Vec<Effect> {
    for _ in 0..n {
        c.perform(Action::Move(motion));
    }
    Vec::new()
}

fn replace_chars(c: &mut Content, ch: char, count: usize) -> Vec<Effect> {
    let at = c.cursor().position;
    let text = line_text(c, at.line);
    let end = advance(&text, at.column, count);
    if text[at.column..end].chars().count() < count {
        return Vec::new(); // vim refuses when there are not enough characters
    }
    select(c, at, Position { line: at.line, column: end });
    c.perform(Action::Edit(Edit::Paste(Arc::new(ch.to_string().repeat(count)))));
    place(c, at);
    vec![Effect::Edited]
}

fn join_lines(c: &mut Content, joins: usize) -> Vec<Effect> {
    let mut edited = false;
    for _ in 0..joins {
        let line = c.cursor().position.line;
        if line + 1 >= c.line_count() {
            break;
        }
        let next = line_text(c, line + 1);
        let blanks = next.len() - next.trim_start().len();
        let current = line_text(c, line);
        let end = Position { line, column: current.trim_end().len() };
        select(c, end, Position { line: line + 1, column: blanks });
        let glue = if current.trim().is_empty() || next.trim().is_empty() { "" } else { " " };
        c.perform(Action::Edit(Edit::Paste(Arc::new(glue.to_string()))));
        edited = true;
    }
    if edited { vec![Effect::Edited] } else { Vec::new() }
}

fn indent(c: &mut Content, first: usize, last: usize, right: bool) -> Vec<Effect> {
    for line in first..=last.min(c.line_count().saturating_sub(1)) {
        let start = Position { line, column: 0 };
        if right {
            place(c, start);
            c.perform(Action::Edit(Edit::Paste(Arc::new("    ".into()))));
        } else {
            let spaces = line_text(c, line).chars().take(4).take_while(|&ch| ch == ' ').count();
            if spaces > 0 {
                select(c, start, Position { line, column: spaces });
                c.perform(Action::Edit(Edit::Delete));
            }
        }
    }
    go(c, Target::Line(first));
    vec![Effect::Edited]
}

fn delete_lines(c: &mut Content, first: usize, last: usize) {
    let total = c.line_count();
    if last + 1 < total {
        select(c, Position { line: first, column: 0 }, Position { line: last + 1, column: 0 });
    } else if first > 0 {
        select(c, end_of(c, first - 1), end_of(c, last));
    } else {
        select(c, Position { line: 0, column: 0 }, end_of(c, last));
    }
    if c.selection().is_some() {
        c.perform(Action::Edit(Edit::Delete));
    }
}

/// Puts the cursor at `p` with no selection. (`move_to` alone keeps an old
/// selection, and a move with any selection only collapses it.)
pub fn place(c: &mut Content, p: Position) {
    c.move_to(Cursor { position: p, selection: Some(p) });
    c.perform(Action::Move(Motion::Left));
}

fn select(c: &mut Content, from: Position, to: Position) {
    c.move_to(Cursor { position: to, selection: Some(from) });
}

fn ordered(a: Position, b: Position) -> (Position, Position) {
    if (a.line, a.column) <= (b.line, b.column) { (a, b) } else { (b, a) }
}

fn line_text(c: &Content, line: usize) -> String {
    c.line(line).map(|l| l.text.into_owned()).unwrap_or_default()
}

fn line_len(c: &Content, line: usize) -> usize {
    c.line(line).map_or(0, |l| l.text.len())
}

fn col(c: &Content) -> usize {
    c.cursor().position.column
}

fn end_of(c: &Content, line: usize) -> Position {
    Position { line, column: line_len(c, line) }
}

/// The position one character after `p` on its line (or `p` at line end).
fn next_char(c: &Content, p: Position) -> Position {
    let text = line_text(c, p.line);
    Position { line: p.line, column: advance(&text, p.column, 1) }
}

fn first_non_blank(text: &str) -> usize {
    text.len() - text.trim_start().len()
}

/// Byte offset `count` characters after `from` (clamped to the line).
fn advance(text: &str, from: usize, count: usize) -> usize {
    let from = from.min(text.len());
    text[from..].char_indices().nth(count).map_or(text.len(), |(i, _)| from + i)
}

/// Byte offset `count` characters before `from`.
fn retreat(text: &str, from: usize, count: usize) -> usize {
    let from = from.min(text.len());
    text[..from].char_indices().rev().nth(count.saturating_sub(1)).map_or(0, |(i, _)| i)
}

fn offset_of(text: &str, p: Position) -> usize {
    let line_start: usize = text.split('\n').take(p.line).map(|l| l.len() + 1).sum();
    (line_start + p.column).min(text.len())
}

fn position_of(text: &str, offset: usize) -> Position {
    let before = &text[..offset];
    let line = before.matches('\n').count();
    let column = offset - before.rfind('\n').map_or(0, |i| i + 1);
    Position { line, column }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(text: &str) -> (Vim, Content) {
        (Vim::default(), Content::with_text(text))
    }

    fn keys(vim: &mut Vim, c: &mut Content, keys: &str) {
        for ch in keys.chars() {
            let key = match ch {
                '\u{1b}' => Key::Esc,
                '\n' => Key::Enter,
                ch => Key::Char(ch),
            };
            vim.key(key, c);
        }
    }

    #[test]
    fn delete_and_paste_lines() {
        let (mut vim, mut c) = editor("one\ntwo\nthree");
        keys(&mut vim, &mut c, "ddp");
        assert_eq!(c.text(), "two\none\nthree");
        keys(&mut vim, &mut c, "G2yyggP");
        assert_eq!(c.text(), "three\ntwo\none\nthree");
    }

    #[test]
    fn delete_last_line() {
        let (mut vim, mut c) = editor("a\nb");
        keys(&mut vim, &mut c, "Gdd");
        assert_eq!(c.text(), "a");
    }

    #[test]
    fn counts_and_char_deletes() {
        let (mut vim, mut c) = editor("abcdef");
        keys(&mut vim, &mut c, "3x");
        assert_eq!(c.text(), "def");
        keys(&mut vim, &mut c, "$p");
        assert_eq!(c.text(), "defabc");
    }

    #[test]
    fn thai_text_is_handled_by_characters() {
        let (mut vim, mut c) = editor("ไทยok");
        keys(&mut vim, &mut c, "2x");
        assert_eq!(c.text(), "ยok");
        keys(&mut vim, &mut c, "rX");
        assert_eq!(c.text(), "Xok");
    }

    #[test]
    fn insert_open_and_change_line() {
        let (mut vim, mut c) = editor("    let x = 1;");
        keys(&mut vim, &mut c, "cc");
        assert_eq!(vim.mode, Mode::Insert);
        assert_eq!(c.text(), "    ");
        keys(&mut vim, &mut c, "\u{1b}");
        assert_eq!(vim.mode, Mode::Normal);
        keys(&mut vim, &mut c, "o");
        assert_eq!(c.text(), "    \n");
        assert_eq!(vim.mode, Mode::Insert);
    }

    #[test]
    fn visual_delete_is_inclusive() {
        let (mut vim, mut c) = editor("hello world");
        keys(&mut vim, &mut c, "vlld");
        assert_eq!(c.text(), "lo world");
        assert_eq!(vim.mode, Mode::Normal);
    }

    #[test]
    fn join_indent_and_search() {
        let (mut vim, mut c) = editor("fn a() {\n    x\n}\nfind me");
        keys(&mut vim, &mut c, "J");
        assert_eq!(c.text(), "fn a() { x\n}\nfind me");
        keys(&mut vim, &mut c, ">>");
        assert_eq!(c.text(), "    fn a() { x\n}\nfind me");
        keys(&mut vim, &mut c, "/me\n");
        assert_eq!(c.cursor().position, Position { line: 2, column: 5 });
    }

    #[test]
    fn colon_commands() {
        let (mut vim, mut c) = editor("a\nb\nc");
        assert_eq!(vim.key(Key::Char(':'), &mut c), vec![]);
        keys(&mut vim, &mut c, "wq");
        assert_eq!(vim.key(Key::Enter, &mut c), vec![Effect::SaveQuit]);
        keys(&mut vim, &mut c, ":3\n");
        assert_eq!(c.cursor().position.line, 2);
    }
}
