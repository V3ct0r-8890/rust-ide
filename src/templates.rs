//! New project templates (File > New project): a Rust or Python project of one
//! of four sizes, written to disk with a name chosen by the user.
//!
//! Everything here is plain text generation, so both front ends share it and
//! the tests can build and run every template.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Rust,
    Python,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    Simple,
    Low,
    Medium,
    Complex,
}

pub const LANGS: [Lang; 2] = [Lang::Rust, Lang::Python];
pub const SIZES: [Size; 4] = [Size::Simple, Size::Low, Size::Medium, Size::Complex];

impl Lang {
    pub fn label(self) -> &'static str {
        match self {
            Lang::Rust => "Rust",
            Lang::Python => "Python",
        }
    }

    /// The word stored in the settings file.
    pub fn setting(self) -> &'static str {
        match self {
            Lang::Rust => "rust",
            Lang::Python => "python",
        }
    }

    pub fn from_setting(value: &str) -> Option<Lang> {
        LANGS.into_iter().find(|l| l.setting() == value)
    }
}

impl Size {
    pub fn label(self) -> &'static str {
        match self {
            Size::Simple => "Simple",
            Size::Low => "Low",
            Size::Medium => "Medium",
            Size::Complex => "Complex",
        }
    }

    pub fn setting(self) -> &'static str {
        match self {
            Size::Simple => "simple",
            Size::Low => "low",
            Size::Medium => "medium",
            Size::Complex => "complex",
        }
    }

    pub fn from_setting(value: &str) -> Option<Size> {
        SIZES.into_iter().find(|s| s.setting() == value)
    }

    /// One line saying what the size contains for a language.
    pub fn describe(self, lang: Lang) -> &'static str {
        match (lang, self) {
            (Lang::Rust, Size::Simple) => "Cargo project with one main.rs (hello world)",
            (Lang::Rust, Size::Low) => "Binary crate in a few modules, with unit tests",
            (Lang::Rust, Size::Medium) => "Library + binary, error handling, config file, integration tests, README",
            (Lang::Rust, Size::Complex) => "Cargo workspace (core, util, cli), tests, benchmark, CI workflow",
            (Lang::Python, Size::Simple) => "One script, main.py",
            (Lang::Python, Size::Low) => "Package with __main__.py and modules, pytest tests",
            (Lang::Python, Size::Medium) => "pyproject.toml, src/ layout, pytest, ruff settings, README",
            (Lang::Python, Size::Complex) => "src/ package with CLI, config, logging, tests, CI workflow",
        }
    }
}

/// The folder name last used for projects, or the home folder.
pub fn default_folder() -> String {
    crate::platform::read_setting("new_project_folder")
        .filter(|f| Path::new(f).is_dir())
        .or_else(|| {
            std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(|h| h.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| ".".to_string())
}

pub fn remember(lang: Lang, size: Size, folder: &str) {
    let _ = crate::platform::write_setting("new_project_lang", lang.setting());
    let _ = crate::platform::write_setting("new_project_size", size.setting());
    let _ = crate::platform::write_setting("new_project_folder", folder);
}

pub fn last_lang() -> Lang {
    crate::platform::read_setting("new_project_lang").and_then(|v| Lang::from_setting(&v)).unwrap_or(Lang::Rust)
}

pub fn last_size() -> Size {
    crate::platform::read_setting("new_project_size").and_then(|v| Size::from_setting(&v)).unwrap_or(Size::Simple)
}

/// Why a project name cannot be used, or `Ok` when it can.
pub fn check_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else { return Err("Type a project name".to_string()) };
    if !first.is_ascii_alphabetic() {
        return Err("The name must start with a letter".to_string());
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("Use only letters, digits, - and _ in the name".to_string());
    }
    const RESERVED: [&str; 12] =
        ["test", "tests", "core", "std", "alloc", "main", "src", "self", "crate", "super", "proc_macro", "build"];
    if RESERVED.contains(&name.to_ascii_lowercase().as_str()) {
        return Err(format!("\"{name}\" is a reserved name, choose another"));
    }
    Ok(())
}

/// What a template generates: relative paths (with `/`) and their text.
pub struct Project {
    pub files: Vec<(String, String)>,
    /// The file opened after creating the project.
    pub main_file: String,
}

/// What [`create`] made.
#[derive(Debug)]
pub struct Created {
    pub dir: PathBuf,
    pub main_file: PathBuf,
}

/// `@NAME@` is the project name; `@IDENT@` the name as an import name: Cargo
/// keeps the case of a crate name, Python packages are lower case.
fn render(text: &str, name: &str, lang: Lang) -> String {
    let ident = name.replace('-', "_");
    let ident = if lang == Lang::Python { ident.to_ascii_lowercase() } else { ident };
    text.replace("@NAME@", name).replace("@IDENT@", &ident)
}

pub fn generate(lang: Lang, size: Size, name: &str) -> Project {
    let (main_file, files): (&str, Vec<(&str, &str)>) = match (lang, size) {
        (Lang::Rust, Size::Simple) => ("src/main.rs", RUST_SIMPLE.to_vec()),
        (Lang::Rust, Size::Low) => ("src/main.rs", RUST_LOW.to_vec()),
        (Lang::Rust, Size::Medium) => ("src/main.rs", RUST_MEDIUM.to_vec()),
        (Lang::Rust, Size::Complex) => ("crates/cli/src/main.rs", RUST_COMPLEX.to_vec()),
        (Lang::Python, Size::Simple) => ("main.py", PY_SIMPLE.to_vec()),
        (Lang::Python, Size::Low) => ("@IDENT@/__main__.py", PY_LOW.to_vec()),
        (Lang::Python, Size::Medium) => ("src/@IDENT@/__main__.py", PY_MEDIUM.to_vec()),
        (Lang::Python, Size::Complex) => ("src/@IDENT@/__main__.py", PY_COMPLEX.to_vec()),
    };
    Project {
        files: files.into_iter().map(|(path, text)| (render(path, name, lang), render(text, name, lang))).collect(),
        main_file: render(main_file, name, lang),
    }
}

/// Writes the project into `parent/name`. The folder must not exist yet, or
/// must be empty.
pub fn create(lang: Lang, size: Size, name: &str, parent: &Path) -> Result<Created, String> {
    check_name(name)?;
    if !parent.is_dir() {
        return Err(format!("{} is not a folder", parent.display()));
    }
    let dir = parent.join(name);
    if dir.exists() {
        let empty = dir.is_dir() && fs::read_dir(&dir).map(|mut d| d.next().is_none()).unwrap_or(false);
        if !empty {
            return Err(format!("{} already exists and is not empty", dir.display()));
        }
    }
    let project = generate(lang, size, name);
    for (path, text) in &project.files {
        let target = path.split('/').fold(dir.clone(), |p, part| p.join(part));
        if let Some(folder) = target.parent() {
            fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        fs::write(&target, text).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    let main_file = project.main_file.split('/').fold(dir.clone(), |p, part| p.join(part));
    Ok(Created { dir, main_file })
}

const GITIGNORE_RUST: &str = "/target\n";

const RUST_SIMPLE: [(&str, &str); 3] = [
    (
        "Cargo.toml",
        r##"[package]
name = "@NAME@"
version = "0.1.0"
edition = "2021"

[dependencies]
"##,
    ),
    (
        "src/main.rs",
        r##"fn main() {
    println!("Hello, world!");
}
"##,
    ),
    (".gitignore", GITIGNORE_RUST),
];

const RUST_LOW: [(&str, &str); 5] = [
    (
        "Cargo.toml",
        r##"[package]
name = "@NAME@"
version = "0.1.0"
edition = "2021"

[dependencies]
"##,
    ),
    (
        "src/main.rs",
        r##"mod greeting;
mod stats;

fn main() {
    println!("{}", greeting::greet("world"));

    let numbers = [3.0, 1.0, 4.0, 1.0, 5.0];
    if let (Some(mean), Some(median)) = (stats::mean(&numbers), stats::median(&numbers)) {
        println!("mean of {numbers:?} = {mean:.2}, median = {median:.2}");
    }
}
"##,
    ),
    (
        "src/greeting.rs",
        r##"/// The greeting for a name.
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greets_by_name() {
        assert_eq!(greet("Ada"), "Hello, Ada!");
    }
}
"##,
    ),
    (
        "src/stats.rs",
        r##"/// The average of the numbers, or `None` for an empty list.
pub fn mean(numbers: &[f64]) -> Option<f64> {
    if numbers.is_empty() {
        None
    } else {
        Some(numbers.iter().sum::<f64>() / numbers.len() as f64)
    }
}

/// The middle value, or `None` for an empty list.
pub fn median(numbers: &[f64]) -> Option<f64> {
    if numbers.is_empty() {
        return None;
    }
    let mut sorted = numbers.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        Some((sorted[middle - 1] + sorted[middle]) / 2.0)
    } else {
        Some(sorted[middle])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_of_numbers() {
        assert_eq!(mean(&[1.0, 2.0, 3.0]), Some(2.0));
        assert_eq!(mean(&[]), None);
    }

    #[test]
    fn median_of_odd_and_even_lists() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }
}
"##,
    ),
    (".gitignore", GITIGNORE_RUST),
];

const RUST_MEDIUM: [(&str, &str); 9] = [
    (
        "Cargo.toml",
        r##"[package]
name = "@NAME@"
version = "0.1.0"
edition = "2021"
description = "A small library with a command line front end"

[dependencies]
"##,
    ),
    (
        "src/lib.rs",
        r##"//! The library behind the `@NAME@` program: everything testable lives here,
//! and `main.rs` only reads the configuration and prints.

pub mod config;
pub mod error;

pub use config::Config;
pub use error::Error;

/// The text the program prints for a configuration.
pub fn run(config: &Config) -> Result<String, Error> {
    if config.repeat == 0 {
        return Err(Error::Invalid("repeat must be at least 1".to_string()));
    }
    Ok(vec![config.greeting.as_str(); config.repeat as usize].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_the_greeting() {
        let config = Config { greeting: "Hi".to_string(), repeat: 2 };
        assert_eq!(run(&config).unwrap(), "Hi\nHi");
    }

    #[test]
    fn zero_repeats_is_an_error() {
        let config = Config { greeting: "Hi".to_string(), repeat: 0 };
        assert!(matches!(run(&config), Err(Error::Invalid(_))));
    }
}
"##,
    ),
    (
        "src/error.rs",
        r##"use std::fmt;

/// Everything that can go wrong in this program.
#[derive(Debug)]
pub enum Error {
    /// The configuration file could not be read.
    Io(std::io::Error),
    /// A line of the configuration file is wrong.
    Parse { line: usize, message: String },
    /// A value is out of range.
    Invalid(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "cannot read the configuration: {e}"),
            Error::Parse { line, message } => write!(f, "configuration line {line}: {message}"),
            Error::Invalid(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
"##,
    ),
    (
        "src/config.rs",
        r##"use std::fs;
use std::path::Path;

use crate::error::Error;

/// Settings read from `config.txt`: one `key = value` per line, `#` starts a comment.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub greeting: String,
    pub repeat: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config { greeting: "Hello, world!".to_string(), repeat: 1 }
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, Error> {
        let mut config = Config::default();
        for (index, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let number = index + 1;
            let Some((key, value)) = line.split_once('=') else {
                return Err(Error::Parse { line: number, message: "expected key = value".to_string() });
            };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "greeting" => config.greeting = value.to_string(),
                "repeat" => {
                    config.repeat = value
                        .parse()
                        .map_err(|_| Error::Parse { line: number, message: format!("\"{value}\" is not a number") })?;
                }
                _ => return Err(Error::Parse { line: number, message: format!("unknown key \"{key}\"") }),
            }
        }
        Ok(config)
    }

    pub fn load(path: &Path) -> Result<Config, Error> {
        Config::parse(&fs::read_to_string(path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keys_comments_and_blank_lines() {
        let config = Config::parse("# comment\n\ngreeting = Hi there\nrepeat = 3 # times\n").unwrap();
        assert_eq!(config, Config { greeting: "Hi there".to_string(), repeat: 3 });
    }

    #[test]
    fn reports_the_line_of_a_mistake() {
        match Config::parse("repeat = 2\nrepeat = many\n") {
            Err(Error::Parse { line, .. }) => assert_eq!(line, 2),
            other => panic!("unexpected {other:?}"),
        }
    }
}
"##,
    ),
    (
        "src/main.rs",
        r##"use std::path::Path;
use std::process::ExitCode;

use @IDENT@::{run, Config};

fn main() -> ExitCode {
    // An optional first argument names the configuration file.
    let path = std::env::args().nth(1).unwrap_or_else(|| "config.txt".to_string());
    let config = if Path::new(&path).exists() {
        Config::load(Path::new(&path))
    } else {
        Ok(Config::default())
    };
    match config.and_then(|config| run(&config)) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
"##,
    ),
    (
        "config.txt",
        r##"# Read by the program from the current folder (or the file named on the command line).
greeting = Hello from the config file!
repeat = 2
"##,
    ),
    (
        "tests/config.rs",
        r##"use @IDENT@::{run, Config, Error};

#[test]
fn a_config_text_drives_the_output() {
    let config = Config::parse("greeting = Hey\nrepeat = 3\n").unwrap();
    assert_eq!(run(&config).unwrap(), "Hey\nHey\nHey");
}

#[test]
fn a_bad_config_is_reported() {
    assert!(matches!(Config::parse("colour = red"), Err(Error::Parse { line: 1, .. })));
}

#[test]
fn a_missing_file_is_an_io_error() {
    let path = std::path::Path::new("this-file-does-not-exist.txt");
    assert!(matches!(Config::load(path), Err(Error::Io(_))));
}
"##,
    ),
    (
        "README.md",
        r##"# @NAME@

A small Rust library with a command line front end.

- `src/lib.rs` - the library: `run` builds the text to print.
- `src/config.rs` - reads `config.txt` (`key = value` lines).
- `src/error.rs` - the error type.
- `src/main.rs` - the program.
- `tests/` - integration tests.

## Use

```
cargo run                # reads config.txt
cargo run -- other.txt   # another configuration file
cargo test
```
"##,
    ),
    (".gitignore", GITIGNORE_RUST),
];

const RUST_COMPLEX: [(&str, &str); 12] = [
    (
        "Cargo.toml",
        r##"[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2021"
"##,
    ),
    (
        "crates/util/Cargo.toml",
        r##"[package]
name = "@NAME@-util"
version.workspace = true
edition.workspace = true
description = "Small text helpers"

[dependencies]
"##,
    ),
    (
        "crates/util/src/lib.rs",
        r##"//! Small text helpers shared by the other crates.

/// The words of a text, split on whitespace.
pub fn words(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}

/// Upper-cases the first letter of every word.
pub fn title_case(text: &str) -> String {
    words(text)
        .into_iter()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_words() {
        assert_eq!(words("  a  b\tc\n"), ["a", "b", "c"]);
    }

    #[test]
    fn title_cases_words() {
        assert_eq!(title_case("hELLO wide world"), "Hello Wide World");
    }
}
"##,
    ),
    (
        "crates/core/Cargo.toml",
        r##"[package]
name = "@NAME@-core"
version.workspace = true
edition.workspace = true
description = "The logic of the program, without any input or output"

[dependencies]
@NAME@-util = { path = "../util" }

[[bench]]
name = "summary"
harness = false
"##,
    ),
    (
        "crates/core/src/lib.rs",
        r##"//! The logic of the program, free of input and output so it is easy to test.

use @IDENT@_util::{title_case, words};

/// Counts about a text.
#[derive(Debug, PartialEq, Eq)]
pub struct Summary {
    pub lines: usize,
    pub words: usize,
    pub longest: String,
}

pub fn summarize(text: &str) -> Summary {
    let all = words(text);
    let longest = all.iter().copied().max_by_key(|w| w.chars().count()).unwrap_or("").to_string();
    Summary { lines: text.lines().count(), words: all.len(), longest }
}

/// The report the command line program prints.
pub fn report(text: &str) -> String {
    let s = summarize(text);
    format!("{} lines, {} words, longest word: {}", s.lines, s.words, title_case(&s.longest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_a_text() {
        let s = summarize("one two\nthree");
        assert_eq!(s, Summary { lines: 2, words: 3, longest: "three".to_string() });
    }

    #[test]
    fn an_empty_text_has_no_words() {
        assert_eq!(summarize("").words, 0);
    }

    #[test]
    fn the_report_names_the_longest_word() {
        assert_eq!(report("a bb ccc"), "1 lines, 3 words, longest word: Ccc");
    }
}
"##,
    ),
    (
        "crates/core/benches/summary.rs",
        r##"//! A plain timing loop (no extra crates needed): `cargo bench`.

use std::hint::black_box;
use std::time::Instant;

use @IDENT@_core::summarize;

fn main() {
    let text = "the quick brown fox jumps over the lazy dog\n".repeat(10_000);
    let runs = 20;
    let start = Instant::now();
    for _ in 0..runs {
        black_box(summarize(black_box(&text)));
    }
    println!("summarize: {:?} per run for {} bytes", start.elapsed() / runs, text.len());
}
"##,
    ),
    (
        "crates/cli/Cargo.toml",
        r##"[package]
name = "@NAME@-cli"
version.workspace = true
edition.workspace = true
description = "The command line program"

[dependencies]
@NAME@-core = { path = "../core" }
"##,
    ),
    (
        "crates/cli/src/main.rs",
        r##"use std::process::ExitCode;

use @IDENT@_core::report;

const SAMPLE: &str = "A sample text.\nGive a file name as the argument to report on your own file.\n";

fn main() -> ExitCode {
    let text = match std::env::args().nth(1) {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("{path}: {e}");
                return ExitCode::FAILURE;
            }
        },
        None => SAMPLE.to_string(),
    };
    println!("{}", report(&text));
    ExitCode::SUCCESS
}
"##,
    ),
    (
        "crates/cli/tests/cli.rs",
        r##"use std::process::Command;

fn program() -> Command {
    Command::new(env!("CARGO_BIN_EXE_@NAME@-cli"))
}

#[test]
fn reports_on_the_sample_text() {
    let output = program().output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("2 lines"));
}

#[test]
fn a_missing_file_fails() {
    let output = program().arg("no-such-file.txt").output().unwrap();
    assert!(!output.status.success());
}
"##,
    ),
    (
        ".github/workflows/ci.yml",
        r##"name: CI

on:
  push:
  pull_request:

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --workspace
      - run: cargo test --workspace
      - run: cargo bench --workspace --no-run
"##,
    ),
    (
        "README.md",
        r##"# @NAME@

A Cargo workspace with three crates:

| Crate | Folder | What it is |
|---|---|---|
| `@NAME@-util` | `crates/util` | text helpers |
| `@NAME@-core` | `crates/core` | the logic (uses util); has a benchmark |
| `@NAME@-cli` | `crates/cli` | the program (uses core); has an integration test |

```
cargo run -p @NAME@-cli             # report on a sample text
cargo run -p @NAME@-cli -- file.txt # report on a file
cargo test --workspace
cargo bench
```

`.github/workflows/ci.yml` builds and tests on Linux and Windows.
"##,
    ),
    (".gitignore", GITIGNORE_RUST),
];

const PY_GITIGNORE: &str = ".venv/\n__pycache__/\n*.pyc\n.pytest_cache/\n.ruff_cache/\n*.egg-info/\ndist/\nbuild/\n";

const PY_SIMPLE: [(&str, &str); 1] = [(
    "main.py",
    r##"def main():
    print("Hello, world!")


if __name__ == "__main__":
    main()
"##,
)];

const PY_LOW: [(&str, &str); 7] = [
    (
        "@IDENT@/__init__.py",
        r##""""@NAME@: a small package."""
"##,
    ),
    (
        "@IDENT@/__main__.py",
        r##""""Run it with F5 or with `python -m @IDENT@`."""

import sys
from pathlib import Path

if __package__ in (None, ""):
    # Started as a plain file (F5): put the project folder on the import path.
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from @IDENT@.greeting import greet  # noqa: E402
from @IDENT@.stats import mean, median  # noqa: E402


def main() -> None:
    print(greet("world"))
    numbers = [3, 1, 4, 1, 5]
    print(f"mean of {numbers} = {mean(numbers):.2f}, median = {median(numbers):.2f}")


if __name__ == "__main__":
    main()
"##,
    ),
    (
        "@IDENT@/greeting.py",
        r##"def greet(name: str) -> str:
    """The greeting for a name."""
    return f"Hello, {name}!"
"##,
    ),
    (
        "@IDENT@/stats.py",
        r##"def mean(numbers: list[float]) -> float:
    """The average of the numbers."""
    if not numbers:
        raise ValueError("no numbers")
    return sum(numbers) / len(numbers)


def median(numbers: list[float]) -> float:
    """The middle value."""
    if not numbers:
        raise ValueError("no numbers")
    ordered = sorted(numbers)
    middle = len(ordered) // 2
    if len(ordered) % 2 == 0:
        return (ordered[middle - 1] + ordered[middle]) / 2
    return ordered[middle]
"##,
    ),
    (
        "tests/test_greeting.py",
        r##"from @IDENT@.greeting import greet


def test_greets_by_name():
    assert greet("Ada") == "Hello, Ada!"
"##,
    ),
    (
        "tests/test_stats.py",
        r##"import pytest

from @IDENT@.stats import mean, median


def test_mean():
    assert mean([1, 2, 3]) == 2


def test_median_of_odd_and_even_lists():
    assert median([3, 1, 2]) == 2
    assert median([4, 1, 3, 2]) == 2.5


def test_empty_list_is_an_error():
    with pytest.raises(ValueError):
        mean([])
"##,
    ),
    (
        "conftest.py",
        "# This file (in the project folder) lets pytest import the package from here.\n",
    ),
];

const PY_MEDIUM: [(&str, &str); 10] = [
    (
        "pyproject.toml",
        r##"[build-system]
requires = ["setuptools>=68"]
build-backend = "setuptools.build_meta"

[project]
name = "@NAME@"
version = "0.1.0"
description = "A small Python project"
readme = "README.md"
requires-python = ">=3.11"
dependencies = []

[project.optional-dependencies]
dev = ["pytest", "ruff"]

[tool.setuptools.packages.find]
where = ["src"]

[tool.pytest.ini_options]
pythonpath = ["src"]
testpaths = ["tests"]

[tool.ruff]
line-length = 100
src = ["src", "tests"]

[tool.ruff.lint]
select = ["E", "F", "I", "B", "UP"]
"##,
    ),
    (
        "src/@IDENT@/__init__.py",
        r##""""@NAME@: a small Python project."""

__version__ = "0.1.0"
"##,
    ),
    (
        "src/@IDENT@/__main__.py",
        r##""""Run it with F5 or with `python -m @IDENT@`."""

import sys
from pathlib import Path

if __package__ in (None, ""):
    # Started as a plain file (F5): put src/ on the import path.
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from @IDENT@.config import load_config  # noqa: E402
from @IDENT@.core import build_message  # noqa: E402


def main() -> None:
    config = load_config()
    print(build_message(config))


if __name__ == "__main__":
    main()
"##,
    ),
    (
        "src/@IDENT@/config.py",
        r##"import tomllib
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Config:
    greeting: str = "Hello, world!"
    repeat: int = 1


def parse_config(text: str) -> Config:
    """Reads the settings from TOML text."""
    data = tomllib.loads(text)
    config = Config(**{k: v for k, v in data.items() if k in Config.__dataclass_fields__})
    if config.repeat < 1:
        raise ValueError("repeat must be at least 1")
    return config


def load_config(path: Path | None = None) -> Config:
    """Reads config.toml from `path`, the current folder or the project folder."""
    project = Path(__file__).resolve().parents[2]
    candidates = [path] if path else [Path("config.toml"), project / "config.toml"]
    for candidate in candidates:
        if candidate.is_file():
            return parse_config(candidate.read_text(encoding="utf-8"))
    return Config()
"##,
    ),
    (
        "src/@IDENT@/core.py",
        r##"from @IDENT@.config import Config


def build_message(config: Config) -> str:
    """The text the program prints."""
    return "\n".join([config.greeting] * config.repeat)
"##,
    ),
    (
        "config.toml",
        "greeting = \"Hello from config.toml!\"\nrepeat = 2\n",
    ),
    (
        "tests/test_config.py",
        r##"import pytest

from @IDENT@.config import Config, load_config, parse_config


def test_parses_toml():
    assert parse_config('greeting = "Hi"\nrepeat = 3\n') == Config("Hi", 3)


def test_repeat_must_be_positive():
    with pytest.raises(ValueError):
        parse_config("repeat = 0")


def test_a_missing_file_gives_defaults(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    assert load_config(tmp_path / "none.toml") == Config()
"##,
    ),
    (
        "tests/test_core.py",
        r##"from @IDENT@.config import Config
from @IDENT@.core import build_message


def test_repeats_the_greeting():
    assert build_message(Config("Hi", 2)) == "Hi\nHi"
"##,
    ),
    (
        "README.md",
        r##"# @NAME@

A small Python project with the `src/` layout.

## Set up

```
python -m venv .venv
.venv\Scripts\activate        # Windows
source .venv/bin/activate     # Linux / macOS
pip install -e ".[dev]"
```

## Use

```
python -m @IDENT@     # or press F5 in the IDE
pytest
ruff check .
```

- `src/@IDENT@/` - the package (`config.py` reads `config.toml`, `core.py` builds the text).
- `tests/` - pytest tests.
- `pyproject.toml` - project, pytest and ruff settings.
"##,
    ),
    (".gitignore", PY_GITIGNORE),
];

const PY_COMPLEX: [(&str, &str); 14] = [
    (
        "pyproject.toml",
        r##"[build-system]
requires = ["setuptools>=68"]
build-backend = "setuptools.build_meta"

[project]
name = "@NAME@"
version = "0.1.0"
description = "A Python application with a command line, configuration and logging"
readme = "README.md"
requires-python = ">=3.11"
dependencies = []

[project.optional-dependencies]
dev = ["pytest", "ruff"]

[project.scripts]
@NAME@ = "@IDENT@.cli:main"

[tool.setuptools.packages.find]
where = ["src"]

[tool.pytest.ini_options]
pythonpath = ["src"]
testpaths = ["tests"]

[tool.ruff]
line-length = 100
src = ["src", "tests"]

[tool.ruff.lint]
select = ["E", "F", "I", "B", "UP"]
"##,
    ),
    (
        "src/@IDENT@/__init__.py",
        r##""""@NAME@: a Python application."""

__version__ = "0.1.0"
"##,
    ),
    (
        "src/@IDENT@/__main__.py",
        r##""""Run it with F5 or with `python -m @IDENT@`."""

import sys
from pathlib import Path

if __package__ in (None, ""):
    # Started as a plain file (F5): put src/ on the import path.
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from @IDENT@.cli import main  # noqa: E402

if __name__ == "__main__":
    sys.exit(main())
"##,
    ),
    (
        "src/@IDENT@/cli.py",
        r##"import argparse
import logging
import sys
from pathlib import Path

from @IDENT@ import __version__
from @IDENT@.config import load_config
from @IDENT@.core import word_report
from @IDENT@.logging_setup import setup_logging

log = logging.getLogger(__name__)

SAMPLE = "A sample text.\nGive a file name to report on your own file.\n"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="@NAME@", description="Reports on the words in a text file.")
    parser.add_argument("file", nargs="?", type=Path, help="text file (a sample text if left out)")
    parser.add_argument("-c", "--config", type=Path, help="configuration file (default: config.toml)")
    parser.add_argument("-v", "--verbose", action="store_true", help="show debug messages")
    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    config = load_config(args.config)
    setup_logging(logging.DEBUG if args.verbose else config.log_level)
    if args.file:
        try:
            text = args.file.read_text(encoding="utf-8")
        except OSError as error:
            print(f"{args.file}: {error.strerror}", file=sys.stderr)
            return 1
    else:
        text = SAMPLE
    log.debug("read %d characters", len(text))
    print(word_report(text, config.top))
    return 0
"##,
    ),
    (
        "src/@IDENT@/config.py",
        r##"import tomllib
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Config:
    top: int = 3
    log_level: str = "INFO"


def parse_config(text: str) -> Config:
    """Reads the settings from TOML text."""
    data = tomllib.loads(text)
    config = Config(**{k: v for k, v in data.items() if k in Config.__dataclass_fields__})
    if config.top < 1:
        raise ValueError("top must be at least 1")
    return config


def load_config(path: Path | None = None) -> Config:
    """Reads config.toml from `path`, the current folder or the project folder."""
    project = Path(__file__).resolve().parents[2]
    candidates = [path] if path else [Path("config.toml"), project / "config.toml"]
    for candidate in candidates:
        if candidate.is_file():
            return parse_config(candidate.read_text(encoding="utf-8"))
    return Config()
"##,
    ),
    (
        "src/@IDENT@/core.py",
        r##"from collections import Counter


def count_words(text: str) -> Counter[str]:
    """How often each word appears, ignoring case and punctuation."""
    cleaned = "".join(c.lower() if c.isalnum() or c.isspace() else " " for c in text)
    return Counter(cleaned.split())


def word_report(text: str, top: int = 3) -> str:
    """The most common words, one per line."""
    counts = count_words(text)
    lines = [f"{len(counts)} different words, {sum(counts.values())} in all"]
    lines += [f"{word}: {number}" for word, number in counts.most_common(top)]
    return "\n".join(lines)
"##,
    ),
    (
        "src/@IDENT@/logging_setup.py",
        r##"import logging


def setup_logging(level: int | str = "INFO") -> None:
    """Logs to the console (stderr) with the time and level."""
    logging.basicConfig(
        level=level,
        format="%(asctime)s %(levelname)-7s %(name)s: %(message)s",
        datefmt="%H:%M:%S",
        force=True,
    )
"##,
    ),
    (
        "config.toml",
        "top = 3\nlog_level = \"INFO\"\n",
    ),
    (
        "tests/test_core.py",
        r##"from @IDENT@.core import count_words, word_report


def test_counts_ignore_case_and_punctuation():
    counts = count_words("The cat. the DOG, the end!")
    assert counts["the"] == 3
    assert counts["cat"] == 1


def test_report_lists_the_top_words():
    report = word_report("a a a b b c", top=2)
    assert report.splitlines() == ["3 different words, 6 in all", "a: 3", "b: 2"]
"##,
    ),
    (
        "tests/test_config.py",
        r##"import pytest

from @IDENT@.config import Config, parse_config


def test_parses_toml():
    assert parse_config('top = 5\nlog_level = "DEBUG"\n') == Config(5, "DEBUG")


def test_top_must_be_positive():
    with pytest.raises(ValueError):
        parse_config("top = 0")
"##,
    ),
    (
        "tests/test_cli.py",
        r##"from @IDENT@.cli import main


def test_reports_on_a_file(tmp_path, capsys):
    path = tmp_path / "text.txt"
    path.write_text("one two two", encoding="utf-8")
    assert main([str(path)]) == 0
    assert "two: 2" in capsys.readouterr().out


def test_a_missing_file_fails(capsys):
    assert main(["no-such-file.txt"]) == 1
    assert "no-such-file.txt" in capsys.readouterr().err


def test_the_sample_text_is_used_without_a_file(capsys):
    assert main([]) == 0
    assert "different words" in capsys.readouterr().out
"##,
    ),
    (
        ".github/workflows/ci.yml",
        r##"name: CI

on:
  push:
  pull_request:

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
        python: ["3.11", "3.13"]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: ${{ matrix.python }}
      - run: pip install -e ".[dev]"
      - run: ruff check .
      - run: pytest
"##,
    ),
    (
        "README.md",
        r##"# @NAME@

A Python application: a command line program with configuration and logging.

## Set up

```
python -m venv .venv
.venv\Scripts\activate        # Windows
source .venv/bin/activate     # Linux / macOS
pip install -e ".[dev]"
```

## Use

```
python -m @IDENT@                 # or press F5 in the IDE
@NAME@ --help                     # after pip install
@NAME@ some-file.txt -v
pytest
ruff check .
```

- `src/@IDENT@/cli.py` - arguments and the `main` entry point.
- `src/@IDENT@/config.py` - reads `config.toml`.
- `src/@IDENT@/core.py` - the word counting.
- `src/@IDENT@/logging_setup.py` - console logging.
- `tests/` - pytest tests.
- `.github/workflows/ci.yml` - runs ruff and pytest on Linux and Windows.
"##,
    ),
    (".gitignore", PY_GITIGNORE),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_checked() {
        assert!(check_name("my-app").is_ok());
        assert!(check_name("My_App2").is_ok());
        assert!(check_name("").is_err());
        assert!(check_name("2fast").is_err());
        assert!(check_name("has space").is_err());
        assert!(check_name("a/b").is_err());
        assert!(check_name("test").is_err());
        assert!(check_name("Core").is_err());
    }

    #[test]
    fn settings_words_round_trip() {
        for lang in LANGS {
            assert_eq!(Lang::from_setting(lang.setting()), Some(lang));
        }
        for size in SIZES {
            assert_eq!(Size::from_setting(size.setting()), Some(size));
        }
        assert_eq!(Size::from_setting("huge"), None);
    }

    #[test]
    fn every_template_names_its_project_and_has_its_main_file() {
        for lang in LANGS {
            for size in SIZES {
                let project = generate(lang, size, "demo-app");
                assert!(
                    project.files.iter().any(|(p, _)| *p == project.main_file),
                    "{lang:?} {size:?}: main file {} is generated",
                    project.main_file
                );
                for (path, text) in &project.files {
                    assert!(!text.contains("@NAME@") && !text.contains("@IDENT@"), "{path}: placeholder left");
                }
                assert!(!size.describe(lang).is_empty());
            }
        }
    }

    #[test]
    fn python_packages_use_underscores() {
        let project = generate(Lang::Python, Size::Medium, "My-App");
        assert_eq!(project.main_file, "src/my_app/__main__.py");
        let project = generate(Lang::Rust, Size::Medium, "My-App");
        let main = project.files.iter().find(|(p, _)| p == "src/main.rs").unwrap();
        assert!(main.1.contains("use My_App::"));
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("templates-test").join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn create_writes_the_files_and_refuses_an_existing_project() {
        let parent = scratch("create");
        let made = create(Lang::Rust, Size::Low, "demo", &parent).unwrap();
        assert_eq!(made.dir, parent.join("demo"));
        assert!(made.main_file.is_file());
        assert!(parent.join("demo").join("src").join("stats.rs").is_file());
        let again = create(Lang::Rust, Size::Low, "demo", &parent);
        assert!(again.unwrap_err().contains("already exists"));
        assert!(create(Lang::Rust, Size::Low, "9bad", &parent).is_err());
        assert!(create(Lang::Rust, Size::Low, "x", &parent.join("missing")).is_err());
        // An empty folder is fine.
        fs::create_dir(parent.join("empty")).unwrap();
        assert!(create(Lang::Python, Size::Simple, "empty", &parent).is_ok());
    }

    /// Builds and tests every Rust template with cargo (needs a toolchain and takes a while).
    #[test]
    #[ignore]
    fn rust_templates_build_run_and_pass_their_tests() {
        let parent = scratch("rust");
        let target = parent.join("shared-target");
        for size in SIZES {
            let name = format!("demo-{}", size.setting());
            let made = create(Lang::Rust, size, &name, &parent).unwrap();
            let cargo = |args: &[&str]| {
                let out = std::process::Command::new("cargo")
                    .args(args)
                    .current_dir(&made.dir)
                    .env("CARGO_TARGET_DIR", &target)
                    .output()
                    .unwrap();
                assert!(
                    out.status.success(),
                    "{size:?}: cargo {args:?}\n{}\n{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
            };
            if size == Size::Complex {
                cargo(&["test", "--workspace", "--offline"]);
                cargo(&["run", "-p", &format!("{name}-cli"), "--offline"]);
                cargo(&["bench", "--workspace", "--no-run", "--offline"]);
            } else {
                cargo(&["test", "--offline"]);
                cargo(&["run", "--offline"]);
            }
        }
    }

    /// Runs every Python template and its pytest tests (needs python and pytest).
    #[test]
    #[ignore]
    fn python_templates_run_and_pass_their_tests() {
        let python = std::env::var("PYTHON").unwrap_or_else(|_| "python".to_string());
        let parent = scratch("python");
        for size in SIZES {
            let name = format!("demo_{}", size.setting());
            let made = create(Lang::Python, size, &name, &parent).unwrap();
            let run = |args: &[&str]| {
                let out = std::process::Command::new(&python).args(args).current_dir(&made.dir).output().unwrap();
                assert!(
                    out.status.success(),
                    "{size:?}: python {args:?}\n{}\n{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
            };
            // Run the main file as F5 does, from the project folder.
            run(&[made.main_file.to_str().unwrap()]);
            if size != Size::Simple {
                run(&["-m", "pytest", "-q", "-p", "no:cacheprovider"]);
            }
        }
    }
}
