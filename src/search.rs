//! Text search used by the Search menu: find (optionally case-insensitive),
//! replace all, and conversions between byte offsets and editor positions.

use iced::widget::text_editor::Position;

/// Finds `query` at or after byte offset `from`, wrapping to the start.
/// Returns the byte range and whether the search wrapped.
pub fn find(text: &str, query: &str, from: usize, case: bool) -> Option<(usize, usize, bool)> {
    if query.is_empty() {
        return None;
    }
    let from = (from.min(text.len())..=text.len()).find(|&i| text.is_char_boundary(i))?;
    if let Some((start, end)) = find_in(text, query, from, text.len(), case) {
        return Some((start, end, false));
    }
    find_in(text, query, 0, from, case).map(|(start, end)| (start, end, true))
}

/// Finds the last match that starts before `before`, wrapping to the end.
pub fn find_back(text: &str, query: &str, before: usize, case: bool) -> Option<(usize, usize, bool)> {
    let matches = all(text, query, case);
    if let Some(&(start, end)) = matches.iter().rev().find(|(start, _)| *start < before) {
        return Some((start, end, false));
    }
    matches.last().map(|&(start, end)| (start, end, true))
}

/// All non-overlapping matches.
pub fn all(text: &str, query: &str, case: bool) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some((start, end)) = find_in(text, query, at, text.len(), case) {
        found.push((start, end));
        at = end;
    }
    found
}

/// Replaces every match; returns the new text and the number of replacements.
pub fn replace_all(text: &str, query: &str, replacement: &str, case: bool) -> (String, usize) {
    let matches = all(text, query, case);
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for &(start, end) in &matches {
        out.push_str(&text[last..start]);
        out.push_str(replacement);
        last = end;
    }
    out.push_str(&text[last..]);
    (out, matches.len())
}

/// True when `candidate` is exactly a match for `query`.
pub fn is_match(candidate: &str, query: &str, case: bool) -> bool {
    !query.is_empty() && match_len(candidate, query, case) == Some(candidate.len())
}

fn find_in(text: &str, query: &str, from: usize, to: usize, case: bool) -> Option<(usize, usize)> {
    if case {
        return text[from..].find(query).map(|i| (from + i, from + i + query.len()))
            .filter(|&(start, _)| start < to);
    }
    text[from..]
        .char_indices()
        .map(|(i, _)| from + i)
        .take_while(|&start| start < to)
        .find_map(|start| match_len(&text[start..], query, false).map(|len| (start, start + len)))
}

/// If `text` starts with `query`, the byte length of the matched part of `text`.
fn match_len(text: &str, query: &str, case: bool) -> Option<usize> {
    if case {
        return text.starts_with(query).then_some(query.len());
    }
    let mut haystack = text.char_indices();
    let mut matched = 0;
    for q in query.chars() {
        let (i, t) = haystack.next()?;
        if !t.to_lowercase().eq(q.to_lowercase()) {
            return None;
        }
        matched = i + t.len_utf8();
    }
    Some(matched)
}

/// Byte offset in the whole text of an editor position (column in bytes).
pub fn offset_of(text: &str, p: Position) -> usize {
    let line_start: usize = text.split('\n').take(p.line).map(|l| l.len() + 1).sum();
    (line_start + p.column).min(text.len())
}

/// Editor position of a byte offset in the whole text.
pub fn position_of(text: &str, offset: usize) -> Position {
    let before = &text[..offset];
    let line = before.matches('\n').count();
    let column = offset - before.rfind('\n').map_or(0, |i| i + 1);
    Position { line, column }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_wraps() {
        let text = "let a = 1;\nlet b = a;";
        assert_eq!(find(text, "a", 0, true), Some((4, 5, false)));
        assert_eq!(find(text, "a", 5, true), Some((19, 20, false)));
        assert_eq!(find(text, "a", 20, true), Some((4, 5, true)));
        assert_eq!(find(text, "let", 12, true), Some((0, 3, true)));
        assert_eq!(find(text, "zzz", 0, true), None);
    }

    #[test]
    fn ignores_case_including_non_ascii() {
        let text = "Hello WORLD, ไทย Straße";
        assert_eq!(find(text, "world", 0, false), Some((6, 11, false)));
        assert_eq!(find(text, "world", 0, true), None);
        let start = text.find("ไทย").unwrap();
        assert_eq!(find(text, "ไทย", 0, false), Some((start, start + "ไทย".len(), false)));
        assert!(find(text, "STRASSE", 0, false).is_none()); // no full case folding
        assert!(find(text, "straße", 0, false).is_some());
    }

    #[test]
    fn finds_backwards() {
        let text = "ab ab ab";
        assert_eq!(find_back(text, "ab", 6, true), Some((3, 5, false)));
        assert_eq!(find_back(text, "ab", 0, true), Some((6, 8, true)));
    }

    #[test]
    fn replaces_all_matches() {
        let (text, n) = replace_all("foo Foo fOO bar", "foo", "x", false);
        assert_eq!((text.as_str(), n), ("x x x bar", 3));
        let (text, n) = replace_all("aaa", "aa", "b", true);
        assert_eq!((text.as_str(), n), ("ba", 1));
    }

    #[test]
    fn offsets_round_trip() {
        let text = "ab\nไทย\nc";
        let p = Position { line: 1, column: 3 };
        let offset = offset_of(text, p);
        assert_eq!(&text[offset..offset + 3], "ท");
        assert_eq!(position_of(text, offset), p);
    }
}
