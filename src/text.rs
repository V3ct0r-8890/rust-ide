//! Text helpers shared by both front ends: tidying empty lines and completing
//! words from the text itself.

use crate::lsp;

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Shrinks every run of empty lines (empty or only spaces) to one. A single empty line stays, so
/// blocks in code keep their spacing. `lines` limits it to line numbers `from..=to`. Returns the
/// new text and how many lines went; the end of the text stays as it was.
pub fn remove_empty_lines(text: &str, lines: Option<(usize, usize)>) -> (String, usize) {
    let parts: Vec<&str> = text.split('\n').collect();
    let last = parts.len() - 1;
    let mut kept = Vec::with_capacity(parts.len());
    let mut removed = 0;
    let mut previous_blank = false;
    for (number, line) in parts.into_iter().enumerate() {
        let blank = number != last && line.trim().is_empty();
        let in_range = lines.is_none_or(|(from, to)| number >= from && number <= to);
        if blank && in_range && previous_blank {
            removed += 1;
        } else {
            kept.push(line);
        }
        previous_blank = blank;
    }
    (kept.join("\n"), removed)
}

/// Words for completion: those already in the text (most used first), then the language's
/// keywords. Only words that start with `prefix`; the word being typed itself is left out.
pub fn local_items(text: &str, keywords: &[&str], prefix: &str) -> Vec<lsp::CompletionItem> {
    let wanted = prefix.to_lowercase();
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for word in text.split(|c: char| !is_ident(c)) {
        let starts_ok = word.chars().next().is_some_and(|c| !c.is_ascii_digit());
        if word.chars().count() >= 2 && starts_ok && word.to_lowercase().starts_with(&wanted) && word != prefix {
            *counts.entry(word).or_default() += 1;
        }
    }
    let mut words: Vec<(&str, usize)> = counts.into_iter().collect();
    words.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let item = |label: &str, detail: &str, kind: &'static str| lsp::CompletionItem {
        label: label.to_string(),
        insert: label.to_string(),
        detail: detail.to_string(),
        kind,
        filter: label.to_string(),
    };
    let mut items: Vec<lsp::CompletionItem> = words.iter().map(|(w, _)| item(w, "in this file", "word")).collect();
    for keyword in keywords {
        if keyword.to_lowercase().starts_with(&wanted) && *keyword != prefix && !items.iter().any(|i| i.label == *keyword) {
            items.push(item(keyword, "keyword", "keyword"));
        }
    }
    items.truncate(60);
    items
}
