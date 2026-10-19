//! The kinds of new file the IDE offers when it starts without a file (and
//! for File > New): the supported programming languages, and plain text
//! and Markdown as the alternatives for everything else.

use std::path::Path;

use crate::platform;

pub struct FileType {
    pub label: &'static str,
    /// File extension without the dot.
    pub ext: &'static str,
    /// The letter that picks it in the chooser (shown in red).
    pub hotkey: char,
}

pub const FILE_TYPES: [FileType; 5] = [
    FileType { label: "Rust", ext: "rs", hotkey: 'r' },
    FileType { label: "Python", ext: "py", hotkey: 'p' },
    FileType { label: "C#", ext: "cs", hotkey: 'c' },
    FileType { label: "Text file", ext: "txt", hotkey: 't' },
    FileType { label: "Markdown", ext: "md", hotkey: 'm' },
];

/// The type of an untitled window nobody has chosen a type for.
pub const DEFAULT_EXT: &str = "rs";

/// Shown in the chooser.
pub const NOTE: &str = "Rust, Python and C# are supported. For any other language use Text or Markdown.";

/// Position of an extension in [`FILE_TYPES`].
pub fn index_of(ext: &str) -> Option<usize> {
    FILE_TYPES.iter().position(|t| t.ext == ext)
}

/// The `'static` extension for a list position or a stored setting value.
pub fn ext_of(index: usize) -> &'static str {
    FILE_TYPES.get(index).map_or(DEFAULT_EXT, |t| t.ext)
}

/// A letter (R, P, T, M) or a digit (1-4) picks a type.
pub fn by_key(c: char) -> Option<usize> {
    let c = c.to_ascii_lowercase();
    match c.to_digit(10) {
        Some(d) if (1..=FILE_TYPES.len() as u32).contains(&d) => Some(d as usize - 1),
        _ => FILE_TYPES.iter().position(|t| t.hotkey == c),
    }
}

/// The type chosen last time (the chooser starts on it); Rust the first time.
pub fn last_choice() -> usize {
    platform::read_setting("new_file_type").and_then(|ext| index_of(&ext)).unwrap_or(0)
}

/// Remembers the choice for next time.
pub fn remember(ext: &str) {
    let _ = platform::write_setting("new_file_type", ext);
}

/// Turbo Pascal's name for a file that has none yet: `NONAME.RS`.
pub fn untitled_name(ext: Option<&str>) -> String {
    format!("NONAME.{}", ext.unwrap_or(DEFAULT_EXT).to_uppercase())
}

/// Adds `.ext` to a typed file name that has no extension.
pub fn with_default_extension(name: &str, ext: &str) -> String {
    let name = name.trim();
    if Path::new(name).extension().is_some() { name.to_string() } else { format!("{name}.{ext}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_has_three_languages_and_two_text_formats() {
        let exts: Vec<_> = FILE_TYPES.iter().map(|t| t.ext).collect();
        assert_eq!(exts, ["rs", "py", "cs", "txt", "md"]);
        assert_eq!(FILE_TYPES[0].ext, DEFAULT_EXT);
        let hotkeys: std::collections::HashSet<_> = FILE_TYPES.iter().map(|t| t.hotkey).collect();
        assert_eq!(hotkeys.len(), FILE_TYPES.len(), "hotkeys are unique");
        for t in &FILE_TYPES {
            assert!(t.label.to_lowercase().contains(t.hotkey), "{} contains its hotkey", t.label);
        }
    }

    #[test]
    fn letters_and_digits_pick_a_type() {
        assert_eq!(by_key('r'), Some(0));
        assert_eq!(by_key('P'), Some(1));
        assert_eq!(by_key('c'), Some(2));
        assert_eq!(by_key('t'), Some(3));
        assert_eq!(by_key('m'), Some(4));
        assert_eq!(by_key('1'), Some(0));
        assert_eq!(by_key('5'), Some(4));
        assert_eq!(by_key('6'), None);
        assert_eq!(by_key('0'), None);
        assert_eq!(by_key('x'), None);
    }

    #[test]
    fn names_and_default_extensions() {
        assert_eq!(untitled_name(None), "NONAME.RS");
        assert_eq!(untitled_name(Some("py")), "NONAME.PY");
        assert_eq!(untitled_name(Some("md")), "NONAME.MD");
        assert_eq!(with_default_extension("notes", "txt"), "notes.txt");
        assert_eq!(with_default_extension("  main.rs ", "py"), "main.rs", "a typed extension is kept");
        assert_eq!(with_default_extension("dir\\prog", "py"), "dir\\prog.py");
        assert_eq!(ext_of(3), "txt");
        assert_eq!(ext_of(99), "rs");
    }

    #[test]
    fn the_last_choice_is_remembered_in_the_settings_file() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("filetype-settings.txt");
        let _ = std::fs::remove_file(&file);
        // SAFETY: only this test uses this variable in the library's test process.
        unsafe { std::env::set_var("RUST_IDE_CONFIG_FILE", &file) };
        assert_eq!(last_choice(), 0, "Rust the first time");
        remember("md");
        assert_eq!(last_choice(), 4);
        remember("py");
        assert_eq!(last_choice(), 1);
        assert!(std::fs::read_to_string(&file).unwrap().contains("new_file_type=py"));
        std::fs::write(&file, "new_file_type=cobol\n").unwrap();
        assert_eq!(last_choice(), 0, "an unknown value falls back to Rust");
        unsafe { std::env::remove_var("RUST_IDE_CONFIG_FILE") };
    }
}
