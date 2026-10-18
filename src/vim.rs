//! A basic Vim emulation on top of iced's text editor: Normal, Insert,
//! Visual and Visual-line modes, counts, d/c/y/>/< operators with motions
//! and text objects, `.` repeat, f/t/F/T with `;` `,`, marks, registers,
//! `/` search and a few `:` commands.
//!
//! Editor columns are byte offsets into the line (as in iced/cosmic-text);
//! helpers below keep every offset on a character boundary.

use std::collections::HashMap;
use std::sync::Arc;

use iced::widget::text_editor::{Action, Content, Cursor, Edit, Motion, Position};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Visual,
    VisualLine,
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
    /// Last `f`/`t`/`F`/`T` and its character, for `;` and `,`.
    last_find: Option<(char, char)>,
    /// Marks `a`-`z` (not adjusted when text changes).
    marks: HashMap<char, Position>,
    /// The last change, for `.`.
    last_change: Option<Change>,
    /// Keys of the command being typed, and whether it is being recorded.
    current: Vec<Key>,
    recording: bool,
    changed: bool,
    insert_start: Option<Position>,
    replaying: bool,
    /// A status message the command wants shown.
    message: Option<String>,
}

/// A repeatable change: the keys that made it and the text typed in Insert mode.
#[derive(Clone)]
struct Change {
    keys: Vec<Key>,
    insert: Option<String>,
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
    /// An absolute position; `true` when an operator includes the character there.
    Pos(Position, bool),
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
            last_find: None,
            marks: HashMap::new(),
            last_change: None,
            current: Vec::new(),
            recording: false,
            changed: false,
            insert_start: None,
            replaying: false,
            message: None,
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
            Mode::VisualLine => "-- VISUAL LINE --",
        };
        if self.pending.is_empty() { mode.into() } else { format!("{mode} {}", self.pending) }
    }

    pub fn key(&mut self, key: Key, c: &mut Content) -> Vec<Effect> {
        if self.replaying {
            return self.dispatch(key, c);
        }
        let idle = self.mode == Mode::Normal && self.pending.is_empty() && self.command.is_none();
        if idle && key == Key::Char('.') {
            return self.repeat(c);
        }
        if idle {
            self.current.clear();
            self.recording = true;
            self.changed = false;
            self.insert_start = None;
        }
        let was_insert = self.mode == Mode::Insert;
        // Insert mode ends here; the cursor then marks the end of the typed text.
        let esc_at = (was_insert && key == Key::Esc).then(|| c.cursor().position);
        if self.recording && !was_insert {
            self.current.push(key);
        }

        let mut effects = self.dispatch(key, c);

        if effects.contains(&Effect::Edited) {
            self.changed = true;
        }
        if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
            self.recording = false;
        }
        if self.recording && !was_insert && self.mode == Mode::Insert {
            self.changed = true;
            self.insert_start = Some(c.cursor().position);
        }
        let mut inserted = None;
        if let Some(end) = esc_at
            && let Some(start) = self.insert_start.take()
        {
            let text = c.text();
            let (a, b) = (offset_of(&text, start), offset_of(&text, end));
            if a <= b && text.is_char_boundary(a) && text.is_char_boundary(b) {
                inserted = Some(text[a..b].to_string());
            }
        }
        if self.recording && self.mode == Mode::Normal && self.pending.is_empty() && self.command.is_none() {
            if self.changed {
                self.last_change = Some(Change { keys: self.current.clone(), insert: inserted });
            }
            self.recording = false;
        }
        if let Some(message) = self.message.take() {
            effects.push(Effect::Status(message));
        }
        effects
    }

    /// `.`: runs the last change again.
    fn repeat(&mut self, c: &mut Content) -> Vec<Effect> {
        let Some(change) = self.last_change.clone() else { return Vec::new() };
        self.replaying = true;
        let mut effects = Vec::new();
        for key in &change.keys {
            effects.extend(self.dispatch(*key, c));
        }
        if self.mode == Mode::Insert {
            if let Some(text) = change.insert.filter(|t| !t.is_empty()) {
                c.perform(Action::Edit(Edit::Paste(Arc::new(text))));
                effects.push(Effect::Edited);
            }
            effects.extend(self.dispatch(Key::Esc, c));
        }
        self.replaying = false;
        self.pending.clear();
        self.command = None;
        self.mode = Mode::Normal;
        if effects.contains(&Effect::Edited) {
            effects.retain(|e| *e != Effect::Edited);
            effects.push(Effect::Edited);
        }
        effects
    }

    fn dispatch(&mut self, key: Key, c: &mut Content) -> Vec<Effect> {
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
            Mode::Visual | Mode::VisualLine => self.visual(key, c),
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

        let (op, prefix) = self.split_pending();
        if prefix == Some('r') {
            let (count, _) = self.take_pending();
            return replace_chars(c, ch, count);
        }
        if prefix == Some('m') {
            self.take_pending();
            self.set_mark(ch, c);
            return Vec::new();
        }
        if prefix.is_none() {
            if ch.is_ascii_digit() && op.is_none() && (ch != '0' || !self.pending.is_empty()) {
                self.pending.push(ch);
                return Vec::new();
            }
            let extends = matches!(ch, 'g' | 'f' | 't' | 'F' | 'T' | '\'' | '`')
                || (op.is_none() && matches!(ch, 'd' | 'c' | 'y' | '>' | '<' | 'r' | 'm'))
                || (op.is_some() && matches!(ch, 'i' | 'a'));
            if extends {
                self.pending.push(ch);
                return Vec::new();
            }
        }

        let (count, has_count) = self.take_pending();
        let line = c.cursor().position.line;

        // Doubled operator: dd, cc, yy, >>, <<
        if let Some(op) = op
            && prefix.is_none()
            && ch == op
        {
            return self.lines_op(op, line, line + count - 1, c);
        }
        if let (Some(op), Some(kind @ ('i' | 'a'))) = (op, prefix) {
            return self.object_op(op, kind, ch, c);
        }

        if let Some(target) = self.motion(prefix, ch, count, has_count, c) {
            return match op {
                None => go(c, target),
                Some(op) => self.operate(op, target, c),
            };
        }
        if op.is_some() || prefix.is_some() {
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
            'V' => {
                self.mode = Mode::VisualLine;
                self.anchor = c.cursor().position;
                self.show_selection(c);
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
        let (_, prefix) = self.split_pending();
        if prefix == Some('m') {
            self.take_pending();
            self.set_mark(ch, c);
            return Vec::new();
        }
        if prefix.is_none() {
            if ch.is_ascii_digit() && (ch != '0' || !self.pending.is_empty()) {
                self.pending.push(ch);
                return Vec::new();
            }
            if matches!(ch, 'g' | 'f' | 't' | 'F' | 'T' | '\'' | '`' | 'm' | 'i' | 'a') {
                self.pending.push(ch);
                return Vec::new();
            }
        }
        let (count, has_count) = self.take_pending();
        if let Some(kind @ ('i' | 'a')) = prefix {
            self.visual_object(kind, ch, c);
            return Vec::new();
        }

        if let Some(target) = self.motion(prefix, ch, count, has_count, c) {
            // Move the free end from the cursor (not from the selection edge).
            let position = c.cursor().position;
            place(c, position);
            go(c, target);
            self.show_selection(c);
            return Vec::new();
        }
        if prefix.is_some() {
            return Vec::new();
        }

        let line_mode = self.mode == Mode::VisualLine;
        let cursor = c.cursor().position;
        let (lo, hi) = (self.anchor.line.min(cursor.line), self.anchor.line.max(cursor.line));
        let (from, to) = self.visual_range(c);
        match ch {
            'd' | 'x' | 'c' | 's' | 'y' if line_mode => {
                let op = match ch {
                    'x' => 'd',
                    's' => 'c',
                    other => other,
                };
                self.mode = Mode::Normal;
                let effects = self.lines_op(op, lo, hi, c);
                if op == 'y' {
                    place(c, Position { line: lo, column: 0 });
                }
                effects
            }
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
                let (first, last) = if line_mode { (lo, hi) } else { (from.line, to.line) };
                let effects = indent(c, first, last, ch == '>');
                self.mode = Mode::Normal;
                effects
            }
            'v' | 'V' => {
                let wanted = if ch == 'v' { Mode::Visual } else { Mode::VisualLine };
                if self.mode == wanted {
                    self.leave_visual(c);
                } else {
                    self.mode = wanted;
                    self.show_selection(c);
                }
                Vec::new()
            }
            'o' => {
                let position = c.cursor().position;
                let other = self.anchor;
                self.anchor = position;
                place(c, other);
                self.show_selection(c);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Splits the pending keys (after any count) into an operator and a prefix key.
    fn split_pending(&self) -> (Option<char>, Option<char>) {
        let rest = self.pending.trim_start_matches(|d: char| d.is_ascii_digit());
        let mut chars = rest.chars();
        match chars.next() {
            Some(op) if matches!(op, 'd' | 'c' | 'y' | '>' | '<') => (Some(op), chars.next()),
            first => (None, first),
        }
    }

    fn set_mark(&mut self, name: char, c: &Content) {
        if name.is_ascii_lowercase() {
            self.marks.insert(name, c.cursor().position);
        }
    }

    /// Resolves a motion: plain keys, `g` motions, `f`/`t`/`F`/`T`, marks, `;` and `,`.
    fn motion(&mut self, prefix: Option<char>, ch: char, count: usize, has_count: bool, c: &Content) -> Option<Target> {
        match prefix {
            Some('g') => return target(ch, true, count, has_count, c),
            Some(kind @ ('f' | 't' | 'F' | 'T')) => {
                self.last_find = Some((kind, ch));
                return find_target(c, kind, ch, count, false);
            }
            Some(kind @ ('\'' | '`')) => {
                let Some(&mark) = self.marks.get(&ch) else {
                    self.message = Some("E20: Mark not set".into());
                    return None;
                };
                let mark = clamp(c, mark);
                return Some(if kind == '\'' { Target::Line(mark.line) } else { Target::Pos(mark, false) });
            }
            Some(_) => return None,
            None => {}
        }
        match ch {
            ';' | ',' => {
                let (kind, found) = self.last_find?;
                let kind = if ch == ',' {
                    match kind {
                        'f' => 'F',
                        'F' => 'f',
                        't' => 'T',
                        _ => 't',
                    }
                } else {
                    kind
                };
                find_target(c, kind, found, count, true)
            }
            _ => target(ch, false, count, has_count, c),
        }
    }

    /// Shows the Visual selection: from the anchor, or whole lines in line mode.
    fn show_selection(&self, c: &mut Content) {
        let position = c.cursor().position;
        let anchor = self.anchor;
        let selection = if self.mode == Mode::VisualLine {
            if (position.line, position.column) >= (anchor.line, anchor.column) {
                Position { line: anchor.line, column: 0 }
            } else {
                end_of(c, anchor.line)
            }
        } else {
            anchor
        };
        c.move_to(Cursor { position, selection: Some(selection) });
    }

    /// `i`/`a` text object after an operator.
    fn object_op(&mut self, op: char, kind: char, obj: char, c: &mut Content) -> Vec<Effect> {
        let text = c.text();
        let here = offset_of(&text, c.cursor().position);
        let Some(found) = text_object(&text, here, kind == 'a', obj) else { return Vec::new() };
        // `ci{` on a multi-line block keeps the line break before the closing brace.
        let to = if found.block_lines && op == 'c' { found.to - 1 } else { found.to };
        let effects = self.operate_range(op, position_of(&text, found.from), position_of(&text, to), c);
        if found.block_lines && op != 'c' {
            self.linewise = true;
        }
        effects
    }

    /// `i`/`a` text object in Visual mode: selects it.
    fn visual_object(&mut self, kind: char, obj: char, c: &mut Content) {
        let text = c.text();
        let here = offset_of(&text, c.cursor().position);
        let Some(found) = text_object(&text, here, kind == 'a', obj) else { return };
        if found.to <= found.from {
            return;
        }
        let Some((last, _)) = text[..found.to].char_indices().next_back() else { return };
        self.mode = Mode::Visual;
        self.anchor = position_of(&text, found.from);
        place(c, position_of(&text, last));
        self.show_selection(c);
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
                let inclusive = matches!(target, Target::Pos(_, true));
                go(c, target);
                let end = c.cursor().position;
                let (from, mut to) = ordered(start, end);
                if inclusive && (end.line, end.column) > (start.line, start.column) {
                    to = next_char(c, to);
                }
                self.operate_range(op, from, to, c)
            }
        }
    }

    /// Applies operator `op` to the character range `from..to`.
    fn operate_range(&mut self, op: char, from: Position, to: Position, c: &mut Content) -> Vec<Effect> {
        if matches!(op, '>' | '<') {
            return indent(c, from.line, to.line, op == '>');
        }
        if from != to {
            select(c, from, to);
            self.register = c.selection().unwrap_or_default();
            self.linewise = false;
        }
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
        Target::Pos(position, _) => place(c, position),
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

/// Keeps a stored position inside the text (marks do not follow edits).
fn clamp(c: &Content, p: Position) -> Position {
    let line = p.line.min(c.line_count().saturating_sub(1));
    let text = line_text(c, line);
    let mut column = p.column.min(text.len());
    while !text.is_char_boundary(column) {
        column -= 1;
    }
    Position { line, column }
}

/// `f`/`t`/`F`/`T` on the current line. `repeat` is `;` / `,`: `t` and `T`
/// then skip a match right next to the cursor.
fn find_target(c: &Content, kind: char, wanted: char, count: usize, repeat: bool) -> Option<Target> {
    let at = c.cursor().position;
    let text = line_text(c, at.line);
    let column = find_char(&text, at.column, kind, wanted, count, repeat)?;
    Some(Target::Pos(Position { line: at.line, column }, matches!(kind, 'f' | 't')))
}

fn find_char(text: &str, from: usize, kind: char, wanted: char, count: usize, repeat: bool) -> Option<usize> {
    let from = from.min(text.len());
    if matches!(kind, 'f' | 't') {
        let start = advance(text, from, 1);
        let next_len = text[start..].chars().next().map_or(0, char::len_utf8);
        let skip_adjacent = repeat && kind == 't';
        let found = text[start..]
            .char_indices()
            .filter(|&(i, ch)| ch == wanted && !(skip_adjacent && i < next_len))
            .nth(count.saturating_sub(1))?
            .0;
        let at = start + found;
        Some(if kind == 'f' { at } else { retreat(text, at, 1) })
    } else {
        let before_len = text[..from].chars().next_back().map_or(0, char::len_utf8);
        let skip_adjacent = repeat && kind == 'T';
        let at = text[..from]
            .char_indices()
            .rev()
            .filter(|&(i, ch)| ch == wanted && !(skip_adjacent && i + before_len == from))
            .nth(count.saturating_sub(1))?
            .0;
        Some(if kind == 'F' { at } else { advance(text, at, 1) })
    }
}

/// A text object found by `text_object`: the byte range `from..to`.
struct Found {
    from: usize,
    to: usize,
    /// The inside of a multi-line block whose closing bracket is on its own
    /// line: the range ends at that line's start, so `d` leaves `{` and `}`.
    block_lines: bool,
}

fn found(from: usize, to: usize) -> Option<Found> {
    Some(Found { from, to, block_lines: false })
}

/// The text object `obj` (the key after `i` / `a`) around byte offset `here`.
fn text_object(text: &str, here: usize, around: bool, obj: char) -> Option<Found> {
    match obj {
        'w' => word_object(text, here, around, false),
        'W' => word_object(text, here, around, true),
        '"' | '\'' | '`' => quote_object(text, here, around, obj),
        '(' | ')' | 'b' => bracket_object(text, here, around, b'(', b')'),
        '[' | ']' => bracket_object(text, here, around, b'[', b']'),
        '{' | '}' | 'B' => bracket_object(text, here, around, b'{', b'}'),
        '<' | '>' => bracket_object(text, here, around, b'<', b'>'),
        _ => None,
    }
}

fn line_bounds(text: &str, here: usize) -> (usize, usize) {
    let start = text[..here].rfind('\n').map_or(0, |i| i + 1);
    let end = text[here..].find('\n').map_or(text.len(), |i| here + i);
    (start, end)
}

fn word_object(text: &str, here: usize, around: bool, big: bool) -> Option<Found> {
    let (ls, le) = line_bounds(text, here);
    let chars: Vec<(usize, char)> = text[ls..le].char_indices().map(|(i, ch)| (ls + i, ch)).collect();
    let at = chars.iter().position(|&(offset, _)| offset == here)?;
    let class = |i: usize| {
        let ch = chars[i].1;
        if ch.is_whitespace() {
            0
        } else if big || !ch.is_ascii_punctuation() || ch == '_' {
            1
        } else {
            2
        }
    };
    let (mut a, mut b) = (at, at);
    while a > 0 && class(a - 1) == class(at) {
        a -= 1;
    }
    while b + 1 < chars.len() && class(b + 1) == class(at) {
        b += 1;
    }
    if around {
        if class(at) == 0 {
            if b + 1 < chars.len() {
                let next = class(b + 1);
                while b + 1 < chars.len() && class(b + 1) == next {
                    b += 1;
                }
            }
        } else if b + 1 < chars.len() && class(b + 1) == 0 {
            while b + 1 < chars.len() && class(b + 1) == 0 {
                b += 1;
            }
        } else {
            while a > 0 && class(a - 1) == 0 {
                a -= 1;
            }
        }
    }
    found(chars[a].0, chars[b].0 + chars[b].1.len_utf8())
}

fn quote_object(text: &str, here: usize, around: bool, quote: char) -> Option<Found> {
    let (ls, le) = line_bounds(text, here);
    let line = &text[ls..le];
    let mut quotes = Vec::new();
    let mut escaped = false;
    for (i, ch) in line.char_indices() {
        if ch == quote && !escaped {
            quotes.push(ls + i);
        }
        escaped = ch == '\\' && !escaped;
    }
    let pair = quotes.chunks_exact(2).find(|pair| pair[1] >= here)?;
    let (open, close) = (pair[0], pair[1]);
    if !around {
        return found(open + 1, close);
    }
    let after = &text[close + 1..le];
    let blanks = after.len() - after.trim_start_matches([' ', '\t']).len();
    if blanks > 0 {
        return found(open, close + 1 + blanks);
    }
    let before = &text[ls..open];
    let blanks = before.len() - before.trim_end_matches([' ', '\t']).len();
    found(open - blanks, close + 1)
}

fn bracket_object(text: &str, here: usize, around: bool, open: u8, close: u8) -> Option<Found> {
    let bytes = text.as_bytes();
    let start = if bytes.get(here) == Some(&open) {
        here
    } else {
        let (mut i, mut depth) = (here, 0usize);
        loop {
            if i == 0 {
                return None;
            }
            i -= 1;
            if bytes[i] == close {
                depth += 1;
            } else if bytes[i] == open {
                if depth == 0 {
                    break i;
                }
                depth -= 1;
            }
        }
    };
    let (mut j, mut depth) = (start + 1, 0usize);
    let end = loop {
        if j >= bytes.len() {
            return None;
        }
        if bytes[j] == open {
            depth += 1;
        } else if bytes[j] == close {
            if depth == 0 {
                break j;
            }
            depth -= 1;
        }
        j += 1;
    };
    if around {
        return found(start, end + 1);
    }
    let from = start + 1;
    if bytes.get(from) == Some(&b'\n') {
        let close_line = text[..end].rfind('\n').map_or(0, |i| i + 1);
        if close_line > from && text[close_line..end].trim().is_empty() {
            return Some(Found { from: from + 1, to: close_line, block_lines: close_line > from + 1 });
        }
    }
    found(from, end)
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

    fn typed(c: &mut Content, text: &str) {
        c.perform(Action::Edit(Edit::Paste(Arc::new(text.to_string()))));
    }

    #[test]
    fn visual_line_mode() {
        let (mut vim, mut c) = editor("a\nb\nc\nd");
        keys(&mut vim, &mut c, "Vjd");
        assert_eq!(c.text(), "c\nd");
        assert_eq!(vim.mode, Mode::Normal);
        keys(&mut vim, &mut c, "p");
        assert_eq!(c.text(), "c\na\nb\nd");
        keys(&mut vim, &mut c, "GVky");
        assert_eq!(vim.register, "b\nd\n");
        assert_eq!(c.cursor().position.line, 2);
        keys(&mut vim, &mut c, "ggV>");
        assert_eq!(c.text(), "    c\na\nb\nd");
        keys(&mut vim, &mut c, "Vv");
        assert_eq!(vim.mode, Mode::Visual);
        keys(&mut vim, &mut c, "\u{1b}");
        keys(&mut vim, &mut c, "Vc");
        assert_eq!(vim.mode, Mode::Insert);
    }

    #[test]
    fn dot_repeats_changes() {
        let (mut vim, mut c) = editor("a b c d e f");
        keys(&mut vim, &mut c, "df ");
        keys(&mut vim, &mut c, ".");
        assert_eq!(c.text(), "c d e f");
        keys(&mut vim, &mut c, "x.");
        assert_eq!(c.text(), "d e f");
        keys(&mut vim, &mut c, "cw");
        typed(&mut c, "XY");
        keys(&mut vim, &mut c, "\u{1b}fe.");
        assert_eq!(c.text(), "XY XY f");
        let (mut vim, mut c) = editor("one\ntwo\nthree");
        keys(&mut vim, &mut c, "A");
        typed(&mut c, "!");
        keys(&mut vim, &mut c, "\u{1b}j.");
        assert_eq!(c.text(), "one!\ntwo!\nthree");
        keys(&mut vim, &mut c, "jdd");
        keys(&mut vim, &mut c, "gg.");
        assert_eq!(c.text(), "two!");
    }

    #[test]
    fn dot_ignores_motions_and_visual() {
        let (mut vim, mut c) = editor("abcdef");
        keys(&mut vim, &mut c, "xlll.");
        assert_eq!(c.text(), "bcdf");
        keys(&mut vim, &mut c, "0vld");
        assert_eq!(c.text(), "df");
        keys(&mut vim, &mut c, ".");
        assert_eq!(c.text(), "f", "dot repeats the last Normal-mode change (x)");
    }

    #[test]
    fn text_objects_words_and_quotes() {
        let (mut vim, mut c) = editor("let foo_bar = \"hi there\";");
        keys(&mut vim, &mut c, "f_ciw");
        typed(&mut c, "x");
        keys(&mut vim, &mut c, "\u{1b}");
        assert_eq!(c.text(), "let x = \"hi there\";");
        keys(&mut vim, &mut c, "f\"ldi\"");
        assert_eq!(c.text(), "let x = \"\";");
        let (mut vim, mut c) = editor("say \"a b\" now");
        keys(&mut vim, &mut c, "da\"");
        assert_eq!(c.text(), "say now");
        let (mut vim, mut c) = editor("one two three");
        keys(&mut vim, &mut c, "fwdaw");
        assert_eq!(c.text(), "one three");
        keys(&mut vim, &mut c, "yiwP");
        assert_eq!(c.text(), "one threethree");
    }

    #[test]
    fn text_objects_brackets() {
        let (mut vim, mut c) = editor("f(a, (b), c)");
        keys(&mut vim, &mut c, "fbdi(");
        assert_eq!(c.text(), "f(a, (), c)");
        keys(&mut vim, &mut c, "da(");
        assert_eq!(c.text(), "f(a, , c)");
        keys(&mut vim, &mut c, "ci(");
        assert_eq!(vim.mode, Mode::Insert);
        assert_eq!(c.text(), "f()");
        let (mut vim, mut c) = editor("fn a() {\n    x;\n    y;\n}");
        keys(&mut vim, &mut c, "jdi{");
        assert_eq!(c.text(), "fn a() {\n}");
        let (mut vim, mut c) = editor("v[1] <T>");
        keys(&mut vim, &mut c, "fTci>");
        assert_eq!(c.text(), "v[1] <>");
        keys(&mut vim, &mut c, "\u{1b}0fdi[");
        assert_eq!(c.text(), "v[1] <>");
    }

    #[test]
    fn visual_text_objects() {
        let (mut vim, mut c) = editor("a (bc) d");
        keys(&mut vim, &mut c, "fbvi(d");
        assert_eq!(c.text(), "a () d");
        let (mut vim, mut c) = editor("one two");
        keys(&mut vim, &mut c, "viwd");
        assert_eq!(c.text(), " two");
    }

    #[test]
    fn find_motions_and_repeat() {
        let (mut vim, mut c) = editor("a,b,c,d");
        keys(&mut vim, &mut c, "f,");
        assert_eq!(col(&c), 1);
        keys(&mut vim, &mut c, ";");
        assert_eq!(col(&c), 3);
        keys(&mut vim, &mut c, ",");
        assert_eq!(col(&c), 1);
        keys(&mut vim, &mut c, "$F,");
        assert_eq!(col(&c), 5);
        keys(&mut vim, &mut c, "0t,");
        assert_eq!(col(&c), 0);
        keys(&mut vim, &mut c, "t,");
        assert_eq!(col(&c), 0);
        keys(&mut vim, &mut c, ";");
        assert_eq!(col(&c), 2, "; after t skips the adjacent match");
        keys(&mut vim, &mut c, "02df,");
        assert_eq!(c.text(), "c,d");
        keys(&mut vim, &mut c, "dt,");
        assert_eq!(c.text(), "c,d", "t next to the cursor does not move");
        keys(&mut vim, &mut c, "df,");
        assert_eq!(c.text(), "d");
        let (mut vim, mut c) = editor("x,y");
        keys(&mut vim, &mut c, "fydF,");
        assert_eq!(c.text(), "xy");
    }

    #[test]
    fn find_motion_dot_and_thai() {
        let (mut vim, mut c) = editor("ไทย:ok:end");
        keys(&mut vim, &mut c, "f:");
        assert_eq!(col(&c), "ไทย".len());
        keys(&mut vim, &mut c, "0dt:");
        assert_eq!(c.text(), ":ok:end");
        keys(&mut vim, &mut c, "l.");
        assert_eq!(c.text(), "::end");
    }

    #[test]
    fn marks() {
        let (mut vim, mut c) = editor("one\n  two\nthree");
        keys(&mut vim, &mut c, "j^ma");
        keys(&mut vim, &mut c, "gg`a");
        assert_eq!(c.cursor().position, Position { line: 1, column: 2 });
        keys(&mut vim, &mut c, "G$'a");
        assert_eq!(c.cursor().position, Position { line: 1, column: 2 });
        keys(&mut vim, &mut c, "gg0d'a");
        assert_eq!(c.text(), "three");
        let effects = vim.key(Key::Char('\''), &mut c);
        assert!(effects.is_empty());
        let effects = vim.key(Key::Char('z'), &mut c);
        assert_eq!(effects, vec![Effect::Status("E20: Mark not set".into())]);
        let (mut vim, mut c) = editor("abcdef");
        keys(&mut vim, &mut c, "llmb0d`b");
        assert_eq!(c.text(), "cdef");
    }
}
