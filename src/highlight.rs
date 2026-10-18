//! Turbo Pascal–style syntax highlighting for Rust and Python, plus
//! red/magenta marks for live LSP diagnostics.

use std::ops::Range;
use std::sync::Arc;

use iced::advanced::text::highlighter::{self, Format};
use iced::{Color, Font, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    Plain,
    Rust,
    Python,
}

impl Lang {
    pub fn from_path(path: Option<&std::path::Path>) -> Self {
        Self::from_extension(path.and_then(|p| p.extension()).and_then(|e| e.to_str()))
    }

    pub fn from_extension(ext: Option<&str>) -> Self {
        match ext {
            Some("rs") => Lang::Rust,
            Some("py" | "pyw") => Lang::Python,
            _ => Lang::Plain,
        }
    }
}

/// A marked range on one line, in character columns: a live diagnostic or
/// a search match.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub line: usize,
    pub start: usize,
    pub end: usize,
    pub kind: MarkKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    Error,
    Warning,
    /// Every search match.
    Match,
    /// The current search match.
    Current,
    /// A line with a breakpoint.
    Breakpoint,
    /// The line the debugger stopped at.
    ExecLine,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub lang: Lang,
    pub marks: Arc<Vec<Mark>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Highlight {
    Keyword,
    Type,
    Str,
    Number,
    Comment,
    Attribute,
    Error,
    Warning,
    Match,
    Current,
    Breakpoint,
    ExecLine,
}

pub fn to_format(highlight: &Highlight, _theme: &Theme) -> Format<Font> {
    let color = match highlight {
        Highlight::Keyword => Color::WHITE,
        Highlight::Type => Color::from_rgb(0.33, 1.0, 0.33),
        Highlight::Str => Color::from_rgb(0.33, 1.0, 1.0),
        Highlight::Number => Color::from_rgb(1.0, 0.6, 1.0),
        Highlight::Comment => Color::from_rgb(0.66, 0.66, 0.66),
        Highlight::Attribute => Color::from_rgb(1.0, 0.75, 0.4),
        Highlight::Error => Color::from_rgb(1.0, 0.3, 0.3),
        Highlight::Warning => Color::from_rgb(1.0, 0.33, 1.0),
        Highlight::Match => Color::from_rgb(1.0, 0.5, 0.0),
        Highlight::Current => Color::WHITE,
        Highlight::Breakpoint => Color::from_rgb(1.0, 0.55, 0.55),
        Highlight::ExecLine => Color::from_rgb(0.35, 1.0, 0.35),
    };
    // Color only: a bold face can have a different width and would break
    // the column alignment of the code.
    Format {
        color: Some(color),
        font: None,
    }
}

/// Lexer state carried from the end of one line to the start of the next.
#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Normal,
    BlockComment(u32),
    Str,
    RawStr(usize),
    Triple(char),
}

pub struct Highlighter {
    settings: Settings,
    /// `states[i]` is the lexer state at the start of line `i`.
    states: Vec<State>,
    current_line: usize,
}

impl highlighter::Highlighter for Highlighter {
    type Settings = Settings;
    type Highlight = Highlight;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Highlight)>;

    fn new(settings: &Settings) -> Self {
        Self {
            settings: settings.clone(),
            states: vec![State::Normal],
            current_line: 0,
        }
    }

    fn update(&mut self, new_settings: &Settings) {
        self.settings = new_settings.clone();
        self.states.truncate(1);
        self.current_line = 0;
    }

    fn change_line(&mut self, line: usize) {
        self.current_line = line.min(self.states.len() - 1);
        self.states.truncate(self.current_line + 1);
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let state = self.states[self.current_line];
        let (tokens, next) = match self.settings.lang {
            Lang::Rust => lex_rust(line, state),
            Lang::Python => lex_python(line, state),
            Lang::Plain => (Vec::new(), State::Normal),
        };
        self.states.truncate(self.current_line + 1);
        self.states.push(next);

        let marks = self.settings.marks.iter().filter(|m| m.line == self.current_line);
        let spans = apply_marks(line, tokens, marks);
        self.current_line += 1;
        spans.into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

/// Highlights lines one after another, keeping multi-line state (block
/// comments, triple-quoted strings). For front ends without iced's editor.
pub struct LineHighlighter {
    lang: Lang,
    state: State,
}

impl LineHighlighter {
    pub fn new(lang: Lang) -> Self {
        Self { lang, state: State::Normal }
    }

    /// Byte ranges and kinds of the tokens in the next line.
    pub fn line(&mut self, line: &str) -> Vec<(Range<usize>, Highlight)> {
        let (tokens, next) = match self.lang {
            Lang::Rust => lex_rust(line, self.state),
            Lang::Python => lex_python(line, self.state),
            Lang::Plain => (Vec::new(), State::Normal),
        };
        self.state = next;
        tokens
    }
}

/// Overlays diagnostic marks on the syntax tokens, producing non-overlapping spans.
fn apply_marks<'a>(
    line: &str,
    tokens: Vec<(Range<usize>, Highlight)>,
    marks: impl Iterator<Item = &'a Mark>,
) -> Vec<(Range<usize>, Highlight)> {
    let mut marks = marks.peekable();
    if marks.peek().is_none() {
        return tokens;
    }

    let mut styles: Vec<Option<Highlight>> = vec![None; line.len()];
    for (range, highlight) in tokens {
        styles[range].fill(Some(highlight));
    }
    let byte_at = |column: usize| line.char_indices().nth(column).map_or(line.len(), |(i, _)| i);
    for mark in marks {
        let start = byte_at(mark.start);
        let end = byte_at(mark.end.max(mark.start + 1)).max(start);
        let highlight = match mark.kind {
            MarkKind::Error => Highlight::Error,
            MarkKind::Warning => Highlight::Warning,
            MarkKind::Match => Highlight::Match,
            MarkKind::Current => Highlight::Current,
            MarkKind::Breakpoint => Highlight::Breakpoint,
            MarkKind::ExecLine => Highlight::ExecLine,
        };
        styles[start..end].fill(Some(highlight));
    }

    let mut spans: Vec<(Range<usize>, Highlight)> = Vec::new();
    for (i, style) in styles.into_iter().enumerate() {
        let Some(style) = style else { continue };
        match spans.last_mut() {
            Some((range, last)) if range.end == i && *last == style => range.end = i + 1,
            _ => spans.push((i..i + 1, style)),
        }
    }
    spans
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
    "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
    "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true",
    "type", "union", "unsafe", "use", "where", "while", "yield",
];
const RUST_TYPES: &[&str] = &[
    "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
    "i128", "isize", "f32", "f64",
];
const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "case", "class",
    "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if",
    "import", "in", "is", "lambda", "match", "nonlocal", "not", "or", "pass", "raise", "return",
    "self", "try", "while", "with", "yield",
];
const PYTHON_TYPES: &[&str] = &[
    "int", "float", "str", "bool", "bytes", "list", "dict", "set", "tuple", "object", "complex",
    "frozenset", "bytearray", "type",
];

struct Lexer<'a> {
    line: &'a str,
    pos: usize,
    tokens: Vec<(Range<usize>, Highlight)>,
}

impl<'a> Lexer<'a> {
    fn new(line: &'a str) -> Self {
        Self {
            line,
            pos: 0,
            tokens: Vec::new(),
        }
    }
    fn rest(&self) -> &'a str {
        &self.line[self.pos..]
    }
    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }
    fn peek_at(&self, n: usize) -> Option<char> {
        self.rest().chars().nth(n)
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }
    fn emit(&mut self, start: usize, highlight: Highlight) {
        if self.pos > start {
            self.tokens.push((start..self.pos, highlight));
        }
    }
    fn to_end(&mut self, highlight: Highlight) {
        let start = self.pos;
        self.pos = self.line.len();
        self.emit(start, highlight);
    }
    fn ident(&mut self) -> &'a str {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            self.bump();
        }
        &self.line[start..self.pos]
    }
    fn number(&mut self) {
        let start = self.pos;
        while let Some(c) = self.peek() {
            let fraction = c == '.' && self.peek_at(1).is_some_and(|d| d.is_ascii_digit());
            if c.is_alphanumeric() || c == '_' || fraction {
                self.bump();
            } else {
                break;
            }
        }
        self.emit(start, Highlight::Number);
    }
    /// Scans a quoted string body up to `quote`; returns false if the line ended first.
    fn quoted(&mut self, quote: char) -> bool {
        while let Some(c) = self.bump() {
            if c == '\\' {
                self.bump();
            } else if c == quote {
                return true;
            }
        }
        false
    }
}

fn lex_rust(line: &str, mut state: State) -> (Vec<(Range<usize>, Highlight)>, State) {
    let mut lx = Lexer::new(line);

    loop {
        let start = lx.pos;
        match state {
            State::BlockComment(mut depth) => {
                while depth > 0 && lx.peek().is_some() {
                    if lx.rest().starts_with("/*") {
                        lx.pos += 2;
                        depth += 1;
                    } else if lx.rest().starts_with("*/") {
                        lx.pos += 2;
                        depth -= 1;
                    } else {
                        lx.bump();
                    }
                }
                lx.emit(start, Highlight::Comment);
                state = if depth == 0 { State::Normal } else { State::BlockComment(depth) };
                if depth > 0 {
                    break;
                }
            }
            State::Str => {
                let closed = lx.quoted('"');
                lx.emit(start, Highlight::Str);
                if !closed {
                    break;
                }
                state = State::Normal;
            }
            State::RawStr(hashes) => {
                let end = format!("\"{}", "#".repeat(hashes));
                match lx.rest().find(&end) {
                    Some(i) => {
                        lx.pos += i + end.len();
                        lx.emit(start, Highlight::Str);
                        state = State::Normal;
                    }
                    None => {
                        lx.to_end(Highlight::Str);
                        break;
                    }
                }
            }
            State::Normal | State::Triple(_) => {
                let Some(c) = lx.peek() else { break };
                let rest = lx.rest();
                if rest.starts_with("//") {
                    lx.to_end(Highlight::Comment);
                } else if rest.starts_with("/*") {
                    lx.pos += 2;
                    lx.emit(start, Highlight::Comment);
                    state = State::BlockComment(1);
                } else if c == '"' {
                    lx.bump();
                    if !lx.quoted('"') {
                        state = State::Str;
                    }
                    lx.emit(start, Highlight::Str);
                } else if (c == 'r' || rest.starts_with("br"))
                    && raw_string_hashes(rest.trim_start_matches('b')).is_some()
                {
                    let prefix = if rest.starts_with("br") { 2 } else { 1 };
                    let hashes = raw_string_hashes(&rest[prefix - 1..]).unwrap_or(0);
                    lx.pos += prefix + hashes + 1;
                    let end = format!("\"{}", "#".repeat(hashes));
                    match lx.rest().find(&end) {
                        Some(i) => lx.pos += i + end.len(),
                        None => {
                            lx.pos = line.len();
                            state = State::RawStr(hashes);
                        }
                    }
                    lx.emit(start, Highlight::Str);
                } else if c == '\'' {
                    lx.bump();
                    let is_char = lx.peek() == Some('\\') || lx.peek_at(1) == Some('\'');
                    if is_char {
                        lx.quoted('\'');
                        lx.emit(start, Highlight::Str);
                    } else {
                        lx.ident(); // lifetime
                        lx.emit(start, Highlight::Attribute);
                    }
                } else if c == '#' && (rest.starts_with("#[") || rest.starts_with("#![")) {
                    let mut depth = 0;
                    while let Some(c) = lx.bump() {
                        match c {
                            '[' => depth += 1,
                            ']' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    lx.emit(start, Highlight::Attribute);
                } else if c.is_ascii_digit() {
                    lx.number();
                } else if c.is_alphabetic() || c == '_' {
                    let word = lx.ident();
                    if lx.peek() == Some('!') && lx.peek_at(1) != Some('=') {
                        lx.bump();
                        lx.emit(start, Highlight::Attribute);
                    } else if RUST_KEYWORDS.contains(&word) {
                        lx.emit(start, Highlight::Keyword);
                    } else if RUST_TYPES.contains(&word) || word.starts_with(char::is_uppercase) {
                        lx.emit(start, Highlight::Type);
                    }
                } else {
                    lx.bump();
                }
            }
        }
    }

    (lx.tokens, state)
}

/// For `r"`, `r#"`, `r##"`... returns the number of hashes.
fn raw_string_hashes(rest: &str) -> Option<usize> {
    let after_r = rest.strip_prefix('r')?;
    let hashes = after_r.chars().take_while(|&c| c == '#').count();
    after_r[hashes..].starts_with('"').then_some(hashes)
}

fn lex_python(line: &str, mut state: State) -> (Vec<(Range<usize>, Highlight)>, State) {
    let mut lx = Lexer::new(line);

    if let State::Triple(quote) = state {
        let end = quote.to_string().repeat(3);
        match line.find(&end) {
            Some(i) => {
                lx.pos = i + 3;
                lx.emit(0, Highlight::Str);
                state = State::Normal;
            }
            None => {
                lx.to_end(Highlight::Str);
                return (lx.tokens, state);
            }
        }
    }

    while let Some(c) = lx.peek() {
        let start = lx.pos;
        if c == '#' {
            lx.to_end(Highlight::Comment);
        } else if c == '@' && line[..start].trim().is_empty() {
            lx.bump();
            while lx.peek().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
                lx.bump();
            }
            lx.emit(start, Highlight::Attribute);
        } else if c.is_ascii_digit() {
            lx.number();
        } else if c.is_alphabetic() || c == '_' || c == '"' || c == '\'' {
            let word = if c == '"' || c == '\'' { "" } else { lx.ident() };
            let is_prefix = word.len() <= 2
                && word.chars().all(|c| "rbfuRBFU".contains(c))
                && matches!(lx.peek(), Some('"' | '\''));
            if word.is_empty() || is_prefix {
                let quote = lx.bump().unwrap_or('"');
                let triple = quote.to_string().repeat(3);
                if lx.rest().starts_with(&triple[1..]) {
                    lx.pos += 2;
                    match lx.rest().find(&triple) {
                        Some(i) => lx.pos += i + 3,
                        None => {
                            lx.pos = line.len();
                            state = State::Triple(quote);
                        }
                    }
                } else {
                    lx.quoted(quote);
                }
                lx.emit(start, Highlight::Str);
            } else if PYTHON_KEYWORDS.contains(&word) {
                lx.emit(start, Highlight::Keyword);
            } else if PYTHON_TYPES.contains(&word) || word.starts_with(char::is_uppercase) {
                lx.emit(start, Highlight::Type);
            }
        } else {
            lx.bump();
        }
    }

    (lx.tokens, state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(tokens: &[(Range<usize>, Highlight)], line: &str) -> Vec<(String, Highlight)> {
        tokens.iter().map(|(r, h)| (line[r.clone()].to_string(), *h)).collect()
    }

    #[test]
    fn rust_tokens() {
        let line = r#"let s: String = format!("hi {}", 42); // done"#;
        let (tokens, state) = lex_rust(line, State::Normal);
        assert_eq!(state, State::Normal);
        assert_eq!(
            kinds(&tokens, line),
            vec![
                ("let".into(), Highlight::Keyword),
                ("String".into(), Highlight::Type),
                ("format!".into(), Highlight::Attribute),
                ("\"hi {}\"".into(), Highlight::Str),
                ("42".into(), Highlight::Number),
                ("// done".into(), Highlight::Comment),
            ]
        );
    }

    #[test]
    fn rust_block_comment_spans_lines() {
        let (_, state) = lex_rust("fn a() {} /* start", State::Normal);
        assert_eq!(state, State::BlockComment(1));
        let (tokens, state) = lex_rust("end */ fn", state);
        assert_eq!(state, State::Normal);
        assert_eq!(tokens[0], (0..6, Highlight::Comment));
        assert_eq!(tokens[1], (7..9, Highlight::Keyword));
    }

    #[test]
    fn rust_char_vs_lifetime() {
        let line = "fn f<'a>(c: char) { let x = 'z'; }";
        let (tokens, _) = lex_rust(line, State::Normal);
        let k = kinds(&tokens, line);
        assert!(k.contains(&("'a".into(), Highlight::Attribute)));
        assert!(k.contains(&("'z'".into(), Highlight::Str)));
    }

    #[test]
    fn python_triple_string_spans_lines() {
        let (tokens, state) = lex_python("x = f\"\"\"hello", State::Normal);
        assert_eq!(state, State::Triple('"'));
        assert_eq!(tokens.last().unwrap().1, Highlight::Str);
        let (tokens, state) = lex_python("world\"\"\" # c", state);
        assert_eq!(state, State::Normal);
        assert_eq!(tokens[0], (0..8, Highlight::Str));
        assert_eq!(tokens[1].1, Highlight::Comment);
    }

    #[test]
    fn marks_override_tokens() {
        let line = "let x = 1;";
        let (tokens, _) = lex_rust(line, State::Normal);
        let mark = Mark { line: 0, start: 4, end: 5, kind: MarkKind::Error };
        let spans = apply_marks(line, tokens, [&mark].into_iter());
        assert!(spans.contains(&(4..5, Highlight::Error)));
        assert!(spans.contains(&(0..3, Highlight::Keyword)));
    }
}
