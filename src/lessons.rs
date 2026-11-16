//! Built-in lessons: 25 short, runnable files each for Rust, Python and C#.
//!
//! The sources live in `lessons/<language>/` and are compiled into the program, so they work from
//! every release package. `open` writes a lesson into the user's data folder (once) and returns
//! its path; the editor then opens it like any other file. Edits the user makes are kept.

use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track {
    Rust,
    Python,
    CSharp,
}

pub struct Lesson {
    /// File name, such as `07_ownership.rs`.
    pub file: &'static str,
    pub source: &'static str,
}

impl Lesson {
    /// The lesson number from the file name (1-based).
    pub fn number(&self) -> usize {
        self.file.split('_').next().and_then(|n| n.parse().ok()).unwrap_or(0)
    }

    /// A readable title: `07_ownership.rs` becomes `Ownership`.
    pub fn title(&self) -> String {
        let stem = self.file.rsplit_once('.').map_or(self.file, |(s, _)| s);
        let words = stem.split_once('_').map_or(stem, |(_, rest)| rest).replace('_', " ");
        let mut chars = words.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        }
    }
}

impl Track {
    pub const ALL: [Track; 3] = [Track::Rust, Track::Python, Track::CSharp];

    pub fn name(self) -> &'static str {
        match self {
            Track::Rust => "Rust",
            Track::Python => "Python",
            Track::CSharp => "C#",
        }
    }

    fn dir(self) -> &'static str {
        match self {
            Track::Rust => "rust",
            Track::Python => "python",
            Track::CSharp => "csharp",
        }
    }

    pub fn lessons(self) -> &'static [Lesson] {
        match self {
            Track::Rust => RUST,
            Track::Python => PYTHON,
            Track::CSharp => CSHARP,
        }
    }
}

/// The folder where lessons are written. `RUST_IDE_LESSONS_DIR` overrides it (used by tests).
pub fn lessons_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("RUST_IDE_LESSONS_DIR") {
        return PathBuf::from(dir);
    }
    crate::platform::data_base("APPDATA", "XDG_DATA_HOME", &[".local", "share"])
        .unwrap_or_else(std::env::temp_dir)
        .join("rust-ide")
        .join("lessons")
}

/// Writes the lesson to disk if it is not there yet and returns its path.
pub fn open(track: Track, index: usize) -> std::io::Result<PathBuf> {
    let lesson = track.lessons().get(index).ok_or_else(|| std::io::Error::other("no such lesson"))?;
    let folder = lessons_dir().join(track.dir());
    std::fs::create_dir_all(&folder)?;
    let path = folder.join(lesson.file);
    if !path.exists() {
        std::fs::write(&path, lesson.source)?;
    }
    Ok(path)
}

static RUST: &[Lesson] = &[
    Lesson { file: "01_hello_world.rs", source: include_str!("../lessons/rust/01_hello_world.rs") },
    Lesson { file: "02_variables_and_types.rs", source: include_str!("../lessons/rust/02_variables_and_types.rs") },
    Lesson { file: "03_numbers_and_math.rs", source: include_str!("../lessons/rust/03_numbers_and_math.rs") },
    Lesson { file: "04_strings.rs", source: include_str!("../lessons/rust/04_strings.rs") },
    Lesson { file: "05_control_flow.rs", source: include_str!("../lessons/rust/05_control_flow.rs") },
    Lesson { file: "06_functions.rs", source: include_str!("../lessons/rust/06_functions.rs") },
    Lesson { file: "07_ownership.rs", source: include_str!("../lessons/rust/07_ownership.rs") },
    Lesson { file: "08_borrowing_and_references.rs", source: include_str!("../lessons/rust/08_borrowing_and_references.rs") },
    Lesson { file: "09_structs_and_methods.rs", source: include_str!("../lessons/rust/09_structs_and_methods.rs") },
    Lesson { file: "10_enums_and_match.rs", source: include_str!("../lessons/rust/10_enums_and_match.rs") },
    Lesson { file: "11_option_and_result.rs", source: include_str!("../lessons/rust/11_option_and_result.rs") },
    Lesson { file: "12_error_handling.rs", source: include_str!("../lessons/rust/12_error_handling.rs") },
    Lesson { file: "13_vectors_and_slices.rs", source: include_str!("../lessons/rust/13_vectors_and_slices.rs") },
    Lesson { file: "14_hashmaps_and_sets.rs", source: include_str!("../lessons/rust/14_hashmaps_and_sets.rs") },
    Lesson { file: "15_iterators_and_closures.rs", source: include_str!("../lessons/rust/15_iterators_and_closures.rs") },
    Lesson { file: "16_traits.rs", source: include_str!("../lessons/rust/16_traits.rs") },
    Lesson { file: "17_generics.rs", source: include_str!("../lessons/rust/17_generics.rs") },
    Lesson { file: "18_lifetimes.rs", source: include_str!("../lessons/rust/18_lifetimes.rs") },
    Lesson { file: "19_modules_and_visibility.rs", source: include_str!("../lessons/rust/19_modules_and_visibility.rs") },
    Lesson { file: "20_smart_pointers.rs", source: include_str!("../lessons/rust/20_smart_pointers.rs") },
    Lesson { file: "21_files_and_input.rs", source: include_str!("../lessons/rust/21_files_and_input.rs") },
    Lesson { file: "22_args_and_environment.rs", source: include_str!("../lessons/rust/22_args_and_environment.rs") },
    Lesson { file: "23_testing.rs", source: include_str!("../lessons/rust/23_testing.rs") },
    Lesson { file: "24_threads_and_channels.rs", source: include_str!("../lessons/rust/24_threads_and_channels.rs") },
    Lesson { file: "25_final_project_inventory.rs", source: include_str!("../lessons/rust/25_final_project_inventory.rs") },
];

static PYTHON: &[Lesson] = &[
    Lesson { file: "01_hello_world.py", source: include_str!("../lessons/python/01_hello_world.py") },
    Lesson { file: "02_variables_and_types.py", source: include_str!("../lessons/python/02_variables_and_types.py") },
    Lesson { file: "03_numbers_and_math.py", source: include_str!("../lessons/python/03_numbers_and_math.py") },
    Lesson { file: "04_strings.py", source: include_str!("../lessons/python/04_strings.py") },
    Lesson { file: "05_control_flow.py", source: include_str!("../lessons/python/05_control_flow.py") },
    Lesson { file: "06_functions.py", source: include_str!("../lessons/python/06_functions.py") },
    Lesson { file: "07_lists_and_tuples.py", source: include_str!("../lessons/python/07_lists_and_tuples.py") },
    Lesson { file: "08_dictionaries_and_sets.py", source: include_str!("../lessons/python/08_dictionaries_and_sets.py") },
    Lesson { file: "09_comprehensions_and_generators.py", source: include_str!("../lessons/python/09_comprehensions_and_generators.py") },
    Lesson { file: "10_classes_and_objects.py", source: include_str!("../lessons/python/10_classes_and_objects.py") },
    Lesson { file: "11_inheritance_and_polymorphism.py", source: include_str!("../lessons/python/11_inheritance_and_polymorphism.py") },
    Lesson { file: "12_error_handling.py", source: include_str!("../lessons/python/12_error_handling.py") },
    Lesson { file: "13_files_and_input.py", source: include_str!("../lessons/python/13_files_and_input.py") },
    Lesson { file: "14_modules_and_packages.py", source: include_str!("../lessons/python/14_modules_and_packages.py") },
    Lesson { file: "15_functions_advanced.py", source: include_str!("../lessons/python/15_functions_advanced.py") },
    Lesson { file: "16_text_and_regex.py", source: include_str!("../lessons/python/16_text_and_regex.py") },
    Lesson { file: "17_dates_and_time.py", source: include_str!("../lessons/python/17_dates_and_time.py") },
    Lesson { file: "18_type_hints.py", source: include_str!("../lessons/python/18_type_hints.py") },
    Lesson { file: "19_testing.py", source: include_str!("../lessons/python/19_testing.py") },
    Lesson { file: "20_logging_and_debugging.py", source: include_str!("../lessons/python/20_logging_and_debugging.py") },
    Lesson { file: "21_args_and_environment.py", source: include_str!("../lessons/python/21_args_and_environment.py") },
    Lesson { file: "22_concurrency.py", source: include_str!("../lessons/python/22_concurrency.py") },
    Lesson { file: "23_standard_library_tour.py", source: include_str!("../lessons/python/23_standard_library_tour.py") },
    Lesson { file: "24_performance_and_idioms.py", source: include_str!("../lessons/python/24_performance_and_idioms.py") },
    Lesson { file: "25_final_project_inventory.py", source: include_str!("../lessons/python/25_final_project_inventory.py") },
];

static CSHARP: &[Lesson] = &[
    Lesson { file: "01_hello_world.cs", source: include_str!("../lessons/csharp/01_hello_world.cs") },
    Lesson { file: "02_variables_and_types.cs", source: include_str!("../lessons/csharp/02_variables_and_types.cs") },
    Lesson { file: "03_numbers_and_math.cs", source: include_str!("../lessons/csharp/03_numbers_and_math.cs") },
    Lesson { file: "04_strings.cs", source: include_str!("../lessons/csharp/04_strings.cs") },
    Lesson { file: "05_control_flow.cs", source: include_str!("../lessons/csharp/05_control_flow.cs") },
    Lesson { file: "06_methods.cs", source: include_str!("../lessons/csharp/06_methods.cs") },
    Lesson { file: "07_arrays_and_lists.cs", source: include_str!("../lessons/csharp/07_arrays_and_lists.cs") },
    Lesson { file: "08_dictionaries_and_sets.cs", source: include_str!("../lessons/csharp/08_dictionaries_and_sets.cs") },
    Lesson { file: "09_linq.cs", source: include_str!("../lessons/csharp/09_linq.cs") },
    Lesson { file: "10_classes_and_objects.cs", source: include_str!("../lessons/csharp/10_classes_and_objects.cs") },
    Lesson { file: "11_inheritance_and_interfaces.cs", source: include_str!("../lessons/csharp/11_inheritance_and_interfaces.cs") },
    Lesson { file: "12_generics.cs", source: include_str!("../lessons/csharp/12_generics.cs") },
    Lesson { file: "13_exceptions.cs", source: include_str!("../lessons/csharp/13_exceptions.cs") },
    Lesson { file: "14_files_and_input.cs", source: include_str!("../lessons/csharp/14_files_and_input.cs") },
    Lesson { file: "15_delegates_and_events.cs", source: include_str!("../lessons/csharp/15_delegates_and_events.cs") },
    Lesson { file: "16_enums_and_records.cs", source: include_str!("../lessons/csharp/16_enums_and_records.cs") },
    Lesson { file: "17_nullability.cs", source: include_str!("../lessons/csharp/17_nullability.cs") },
    Lesson { file: "18_dates_and_text.cs", source: include_str!("../lessons/csharp/18_dates_and_text.cs") },
    Lesson { file: "19_interfaces_and_dependency_injection.cs", source: include_str!("../lessons/csharp/19_interfaces_and_dependency_injection.cs") },
    Lesson { file: "20_async_await.cs", source: include_str!("../lessons/csharp/20_async_await.cs") },
    Lesson { file: "21_threads_and_parallelism.cs", source: include_str!("../lessons/csharp/21_threads_and_parallelism.cs") },
    Lesson { file: "22_testing.cs", source: include_str!("../lessons/csharp/22_testing.cs") },
    Lesson { file: "23_args_and_environment.cs", source: include_str!("../lessons/csharp/23_args_and_environment.cs") },
    Lesson { file: "24_project_structure_and_nuget.cs", source: include_str!("../lessons/csharp/24_project_structure_and_nuget.cs") },
    Lesson { file: "25_final_project_inventory.cs", source: include_str!("../lessons/csharp/25_final_project_inventory.cs") },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_track_has_25_numbered_lessons_in_order() {
        for track in Track::ALL {
            let lessons = track.lessons();
            assert_eq!(lessons.len(), 25, "{}", track.name());
            for (i, lesson) in lessons.iter().enumerate() {
                assert_eq!(lesson.number(), i + 1, "{}", lesson.file);
                assert!(!lesson.title().is_empty());
                assert!(lesson.source.lines().next().unwrap_or("").contains(&format!("Lesson {}:", i + 1)), "{}", lesson.file);
            }
        }
    }

    #[test]
    fn titles_are_readable() {
        let lesson = &Track::Rust.lessons()[6];
        assert_eq!(lesson.file, "07_ownership.rs");
        assert_eq!(lesson.title(), "Ownership");
        assert_eq!(Track::Python.lessons()[0].title(), "Hello world");
    }

    #[test]
    fn file_extensions_match_the_language() {
        for (track, ext) in [(Track::Rust, ".rs"), (Track::Python, ".py"), (Track::CSharp, ".cs")] {
            assert!(track.lessons().iter().all(|l| l.file.ends_with(ext)));
        }
    }

    #[test]
    fn open_writes_once_and_keeps_edits() {
        let dir = std::env::temp_dir().join(format!("rust-ide-lessons-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // Only this test sets the variable, and it restores nothing: the process ends with the tests.
        unsafe { std::env::set_var("RUST_IDE_LESSONS_DIR", &dir) };
        let path = open(Track::Python, 0).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), Track::Python.lessons()[0].source);
        assert!(path.ends_with("python/01_hello_world.py") || path.ends_with("python\\01_hello_world.py"));
        std::fs::write(&path, "my notes").unwrap();
        assert_eq!(open(Track::Python, 0).unwrap(), path);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "my notes");
        assert!(open(Track::Rust, 99).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Compiles and runs every Rust lesson. Needs rustc: `cargo test lessons:: -- --ignored`.
    #[test]
    #[ignore]
    fn rust_lessons_compile_and_run() {
        let dir = std::env::temp_dir().join(format!("rust-ide-lessons-rs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for lesson in Track::Rust.lessons() {
            let source = dir.join(lesson.file);
            std::fs::write(&source, lesson.source).unwrap();
            let exe = dir.join(format!("{}{}", lesson.file, std::env::consts::EXE_SUFFIX));
            let build = std::process::Command::new("rustc")
                .args(["--edition", "2024", "-o"]).arg(&exe).arg(&source).output().unwrap();
            assert!(build.status.success(), "{}: {}", lesson.file, String::from_utf8_lossy(&build.stderr));
            let run = std::process::Command::new(&exe).stdin(std::process::Stdio::null()).output().unwrap();
            assert!(run.status.success(), "{}: {}", lesson.file, String::from_utf8_lossy(&run.stderr));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Runs every Python lesson. Needs python on the PATH.
    #[test]
    #[ignore]
    fn python_lessons_run() {
        let dir = std::env::temp_dir().join(format!("rust-ide-lessons-py-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let python = if cfg!(windows) { "python" } else { "python3" };
        for lesson in Track::Python.lessons() {
            let source = dir.join(lesson.file);
            std::fs::write(&source, lesson.source).unwrap();
            let run = std::process::Command::new(python).arg(&source).stdin(std::process::Stdio::null()).output().unwrap();
            assert!(run.status.success(), "{}: {}", lesson.file, String::from_utf8_lossy(&run.stderr));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Runs every C# lesson with `dotnet run file.cs` (.NET 10).
    #[test]
    #[ignore]
    fn csharp_lessons_run() {
        let dir = std::env::temp_dir().join(format!("rust-ide-lessons-cs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for lesson in Track::CSharp.lessons() {
            let source = dir.join(lesson.file);
            std::fs::write(&source, lesson.source).unwrap();
            let run = std::process::Command::new("dotnet").arg("run").arg(&source).stdin(std::process::Stdio::null()).output().unwrap();
            assert!(run.status.success(), "{}: {}", lesson.file, String::from_utf8_lossy(&run.stdout));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
