//! Draws Markdown as styled, wrapped terminal lines for the preview.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// The colors of the preview, chosen by the theme.
#[derive(Clone, Copy)]
pub struct Styles {
    pub text: Style,
    pub heading: Style,
    pub code: Style,
    pub link: Style,
    pub quote: Style,
}

/// Lines of the page, wrapped to `width` columns.
pub fn render(source: &str, width: usize, styles: Styles) -> Vec<Line<'static>> {
    let mut page = Page { lines: Vec::new(), words: Vec::new(), styles, width: width.max(10), ..Page::default() };
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for event in Parser::new_ext(source, options) {
        page.event(event);
    }
    page.flush();
    while page.lines.last().is_some_and(|l| l.spans.is_empty()) {
        page.lines.pop();
    }
    page.lines
}

#[derive(Default)]
struct Page {
    lines: Vec<Line<'static>>,
    /// The paragraph being collected: text pieces and their styles.
    words: Vec<(String, Style)>,
    styles: Styles,
    width: usize,
    /// Bold, italic, ... from the open inline tags.
    inline: Vec<Modifier>,
    heading: Option<HeadingLevel>,
    link: usize,
    code_block: bool,
    quote: usize,
    /// Open lists: the next number, or None for bullets.
    lists: Vec<Option<u64>>,
    /// The marker of the list item whose first line is being collected.
    marker: Option<String>,
    /// Cells of the table row being read, and the rows so far.
    cell: String,
    row: Vec<String>,
    table: Vec<Vec<String>>,
    in_table: bool,
}

impl Default for Styles {
    fn default() -> Self {
        Self { text: Style::new(), heading: Style::new(), code: Style::new(), link: Style::new(), quote: Style::new() }
    }
}

impl Page {
    fn style(&self) -> Style {
        let mut style = if self.heading.is_some() {
            self.styles.heading
        } else if self.link > 0 {
            self.styles.link
        } else if self.quote > 0 {
            self.styles.quote
        } else {
            self.styles.text
        };
        for modifier in &self.inline {
            style = style.add_modifier(*modifier);
        }
        style
    }

    /// Spaces (and quote bars) in front of every line.
    fn indent(&self) -> String {
        let mut indent = "\u{2502} ".repeat(self.quote);
        indent.push_str(&"   ".repeat(self.lists.len().saturating_sub(1)));
        indent
    }

    fn blank(&mut self) {
        if self.lines.last().is_some_and(|l| !l.spans.is_empty()) {
            self.lines.push(Line::default());
        }
    }

    fn event(&mut self, event: Event) {
        if self.in_table {
            match event {
                Event::Text(text) | Event::Code(text) => self.cell.push_str(&text),
                Event::End(TagEnd::TableCell) => self.row.push(std::mem::take(&mut self.cell)),
                Event::End(TagEnd::TableHead | TagEnd::TableRow) => self.table.push(std::mem::take(&mut self.row)),
                Event::End(TagEnd::Table) => {
                    self.in_table = false;
                    self.draw_table();
                }
                _ => {}
            }
            return;
        }
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    self.flush();
                    self.blank();
                    self.heading = Some(level);
                }
                Tag::Paragraph => {}
                Tag::BlockQuote(_) => {
                    self.flush();
                    self.quote += 1;
                }
                Tag::CodeBlock(_) => {
                    self.flush();
                    self.blank();
                    self.code_block = true;
                }
                Tag::List(start) => {
                    self.flush();
                    if self.lists.is_empty() {
                        self.blank();
                    }
                    self.lists.push(start);
                }
                Tag::Item => {
                    self.flush();
                    let marker = match self.lists.last_mut() {
                        Some(Some(number)) => {
                            *number += 1;
                            format!("{}. ", *number - 1)
                        }
                        _ => "\u{2022} ".to_string(),
                    };
                    self.marker = Some(marker);
                }
                Tag::Emphasis => self.inline.push(Modifier::ITALIC),
                Tag::Strong => self.inline.push(Modifier::BOLD),
                Tag::Strikethrough => self.inline.push(Modifier::CROSSED_OUT),
                Tag::Link { .. } => self.link += 1,
                Tag::Image { .. } => self.words.push(("[image: ".into(), self.styles.link)),
                Tag::Table(_) => {
                    self.flush();
                    self.blank();
                    self.in_table = true;
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    self.flush();
                    self.heading = None;
                    self.lines.push(Line::default());
                }
                TagEnd::Paragraph => {
                    self.flush();
                    if self.lists.is_empty() {
                        self.lines.push(Line::default());
                    }
                }
                TagEnd::BlockQuote(_) => {
                    self.flush();
                    self.quote = self.quote.saturating_sub(1);
                    self.blank();
                }
                TagEnd::CodeBlock => {
                    self.code_block = false;
                    self.lines.push(Line::default());
                }
                TagEnd::List(_) => {
                    self.flush();
                    self.lists.pop();
                    if self.lists.is_empty() {
                        self.lines.push(Line::default());
                    }
                }
                TagEnd::Item => self.flush(),
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                    self.inline.pop();
                }
                TagEnd::Link => self.link = self.link.saturating_sub(1),
                TagEnd::Image => self.words.push(("]".into(), self.styles.link)),
                _ => {}
            },
            Event::Text(text) if self.code_block => {
                let indent = self.indent();
                for line in text.trim_end_matches('\n').split('\n') {
                    let shown: String = line.chars().take(self.width.saturating_sub(indent.chars().count() + 2)).collect();
                    self.lines.push(Line::from(vec![
                        Span::styled(format!("{indent}  "), self.styles.text),
                        Span::styled(shown, self.styles.code),
                    ]));
                }
            }
            Event::Text(text) => {
                let style = self.style();
                self.words.push((text.into_string(), style));
            }
            Event::Code(text) => self.words.push((text.into_string(), self.styles.code)),
            Event::SoftBreak => {
                let style = self.style();
                self.words.push((" ".into(), style));
            }
            Event::HardBreak => self.flush(),
            Event::Rule => {
                self.flush();
                self.lines.push(Line::styled("\u{2500}".repeat(self.width), self.styles.quote));
                self.lines.push(Line::default());
            }
            Event::TaskListMarker(done) => self.words.push((if done { "[x] " } else { "[ ] " }.into(), self.styles.text)),
            Event::Html(html) | Event::InlineHtml(html) => self.words.push((html.into_string(), self.styles.quote)),
            _ => {}
        }
    }

    /// Wraps the collected paragraph into lines.
    fn flush(&mut self) {
        let marker = self.marker.take();
        if self.words.is_empty() {
            if let Some(marker) = marker {
                // An item whose text comes in a nested block: keep its marker.
                self.marker = Some(marker);
            }
            return;
        }
        let indent = self.indent();
        let first = format!("{indent}{}", marker.clone().unwrap_or_default());
        let rest = format!("{indent}{}", " ".repeat(marker.as_deref().map_or(0, |m| m.chars().count())));
        let room = self.width.saturating_sub(first.chars().count()).max(8);

        // Split into words that keep their trailing spaces, so styles survive wrapping.
        let mut pieces: Vec<(String, Style)> = Vec::new();
        for (text, style) in std::mem::take(&mut self.words) {
            let mut word = String::new();
            for c in text.chars() {
                word.push(c);
                if c == ' ' {
                    pieces.push((std::mem::take(&mut word), style));
                }
            }
            if !word.is_empty() {
                pieces.push((word, style));
            }
        }
        let mut line: Vec<Span<'static>> = vec![Span::styled(first, self.styles.text)];
        let mut used = 0;
        for (mut word, style) in pieces {
            let len = word.trim_end().chars().count();
            if used > 0 && used + len > room {
                self.lines.push(finish(std::mem::take(&mut line)));
                line.push(Span::styled(rest.clone(), self.styles.text));
                used = 0;
            }
            if used == 0 && word.trim().is_empty() {
                continue;
            }
            // A word longer than the line is cut.
            while word.chars().count() > room {
                let head: String = word.chars().take(room).collect();
                word = word.chars().skip(room).collect();
                line.push(Span::styled(head, style));
                self.lines.push(finish(std::mem::take(&mut line)));
                line.push(Span::styled(rest.clone(), self.styles.text));
            }
            used += word.chars().count();
            line.push(Span::styled(word, style));
        }
        if let Some(level) = self.heading {
            let modifier = if level == HeadingLevel::H1 { Modifier::BOLD | Modifier::UNDERLINED } else { Modifier::BOLD };
            line = line.into_iter().map(|s| s.patch_style(Style::new().add_modifier(modifier))).collect();
        }
        self.lines.push(finish(line));
    }

    fn draw_table(&mut self) {
        let table = std::mem::take(&mut self.table);
        let columns = table.iter().map(Vec::len).max().unwrap_or(0);
        let widths: Vec<usize> =
            (0..columns).map(|c| table.iter().filter_map(|r| r.get(c)).map(|s| s.chars().count()).max().unwrap_or(0)).collect();
        for (index, row) in table.iter().enumerate() {
            let cells: Vec<String> =
                (0..columns).map(|c| format!("{:<w$}", row.get(c).map_or("", String::as_str), w = widths[c])).collect();
            let text: String = cells.join(" \u{2502} ").chars().take(self.width).collect();
            let style = if index == 0 { self.styles.text.add_modifier(Modifier::BOLD) } else { self.styles.text };
            self.lines.push(Line::styled(text, style));
            if index == 0 {
                let rule: Vec<String> = widths.iter().map(|w| "\u{2500}".repeat(*w)).collect();
                let rule: String = rule.join("\u{2500}\u{253C}\u{2500}").chars().take(self.width).collect();
                self.lines.push(Line::styled(rule, self.styles.quote));
            }
        }
        self.lines.push(Line::default());
    }
}

/// A wrapped line without the space it was broken at.
fn finish(mut spans: Vec<Span<'static>>) -> Line<'static> {
    while spans.len() > 1 && spans.last().is_some_and(|s| s.content.trim_end().is_empty()) {
        spans.pop();
    }
    if let Some(last) = spans.last_mut() {
        last.content = last.content.trim_end().to_string().into();
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
    }

    #[test]
    fn headings_lists_code_and_wrapping() {
        let text = "# Title\n\nSome *words* here that wrap around.\n\n- one\n- two\n\n1. first\n2. second\n\n```\nlet x = 1;\n```\n\n> quoted\n";
        let lines = render(text, 20, Styles::default());
        let shown = plain(&lines);
        assert_eq!(shown[0], "Title");
        assert!(lines[0].spans.iter().any(|s| s.style.add_modifier.contains(Modifier::UNDERLINED)), "{shown:?}");
        assert!(shown.contains(&"Some words here that".to_string()) || shown.contains(&"Some words here".to_string()), "{shown:?}");
        assert!(shown.iter().any(|l| l.trim_end() == "wrap around."), "{shown:?}");
        assert!(shown.contains(&"\u{2022} one".to_string()), "{shown:?}");
        assert!(shown.contains(&"2. second".to_string()), "{shown:?}");
        assert!(shown.contains(&"  let x = 1;".to_string()), "{shown:?}");
        assert!(shown.contains(&"\u{2502} quoted".to_string()), "{shown:?}");
    }

    #[test]
    fn tables_line_up() {
        let lines = render("| a | bb |\n|---|---|\n| ccc | d |\n", 40, Styles::default());
        let shown = plain(&lines);
        assert_eq!(shown[0], "a   \u{2502} bb");
        assert_eq!(shown[2], "ccc \u{2502} d ");
    }
}
