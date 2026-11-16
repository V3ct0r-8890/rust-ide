//! The open files of a project, remembered when the IDE closes and opened
//! again next time: which files, where the cursor was, which window was active.
//!
//! One small file per project folder in the IDE's own settings folder
//! (`sessions/`), so nothing is added to the project. The project folder last
//! used is remembered too, for a start from a shortcut whose working folder
//! says nothing.

use std::path::{Path, PathBuf};

use crate::{env, platform};

/// One open file and where its cursor was (0-based line, column in characters).
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Session {
    pub files: Vec<Entry>,
    /// Index into `files` of the window that was active.
    pub active: usize,
}

/// Where the session of the project at `root` is kept.
pub fn session_file(root: &Path) -> Option<PathBuf> {
    let text = root.to_string_lossy().to_lowercase();
    // The folder's name for readability, a hash of the whole path to tell equal names apart.
    let hash = text.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3));
    let name: String = root
        .file_name()
        .map(|n| n.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect())
        .unwrap_or_else(|| "root".to_string());
    Some(platform::config_path()?.with_file_name("sessions").join(format!("{name}-{hash:016x}.session")))
}

fn render(session: &Session) -> String {
    let mut out = format!("active={}\n", session.active);
    for e in &session.files {
        out.push_str(&format!("file={}\t{}\t{}\n", e.line, e.column, e.path.display()));
    }
    out
}

fn parse(text: &str) -> Session {
    let mut session = Session::default();
    for line in text.lines() {
        if let Some(n) = line.strip_prefix("active=") {
            session.active = n.trim().parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("file=") {
            let mut parts = rest.splitn(3, '\t');
            let (Some(l), Some(c), Some(path)) = (parts.next(), parts.next(), parts.next()) else { continue };
            if let (Ok(line), Ok(column)) = (l.parse(), c.parse()) {
                session.files.push(Entry { path: PathBuf::from(path), line, column });
            }
        }
    }
    session
}

/// The project folder a session is filed under: that of the first file.
fn root_of(session: &Session) -> Option<PathBuf> {
    let first = session.files.first()?;
    Some(env::project_root(first.path.parent()?))
}

/// Remembers `session` for its project (nothing open removes the old one) and
/// makes it the one to restore when the IDE starts somewhere unrelated.
pub fn save(session: &Session) -> std::io::Result<()> {
    let Some(root) = root_of(session) else { return Ok(()) };
    let file = session_file(&root).ok_or_else(|| std::io::Error::other("no config directory"))?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(file, render(session))?;
    platform::write_setting("last_session", &root.to_string_lossy())
}

/// Forgets the session of the project at `root`.
pub fn forget(root: &Path) {
    if let Some(file) = session_file(root) {
        let _ = std::fs::remove_file(file);
    }
}

/// The session of the project at `root`, keeping only files that still exist.
pub fn load(root: &Path) -> Option<Session> {
    let text = std::fs::read_to_string(session_file(root)?).ok()?;
    let mut session = parse(&text);
    let active_path = session.files.get(session.active).map(|e| e.path.clone());
    session.files.retain(|e| e.path.is_file());
    session.active = active_path.and_then(|p| session.files.iter().position(|e| e.path == p)).unwrap_or(0);
    (!session.files.is_empty()).then_some(session)
}

/// What to reopen when the IDE starts without files: the session of the
/// project of the working folder `cwd`, else the one used last.
pub fn startup(cwd: &Path) -> Option<Session> {
    load(&env::project_root(cwd)).or_else(|| load(Path::new(&platform::read_setting("last_session")?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_is_saved_per_project_and_comes_back_without_missing_files() {
        static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let base = std::env::temp_dir().join("rust-ide-session-test");
        let _ = std::fs::remove_dir_all(&base);
        let (one, two) = (base.join("one"), base.join("two"));
        for project in [&one, &two] {
            std::fs::create_dir_all(project.join(".git")).unwrap();
        }
        let (a, b, gone) = (one.join("a.rs"), one.join("b.py"), one.join("gone.rs"));
        for f in [&a, &b, &gone] {
            std::fs::write(f, "x\n").unwrap();
        }
        let other = two.join("c.rs");
        std::fs::write(&other, "x\n").unwrap();
        // SAFETY: only this test uses the variable (it takes the lock above).
        unsafe { std::env::set_var("RUST_IDE_CONFIG_FILE", base.join("settings.txt")) };

        let session = Session {
            files: vec![
                Entry { path: a.clone(), line: 4, column: 2 },
                Entry { path: gone.clone(), line: 0, column: 0 },
                Entry { path: b.clone(), line: 10, column: 7 },
            ],
            active: 2,
        };
        assert_eq!(parse(&render(&session)), session, "the text form keeps everything");
        save(&session).unwrap();

        std::fs::remove_file(&gone).unwrap();
        let back = load(&one).expect("the project's session");
        assert_eq!(back.files.iter().map(|e| e.path.clone()).collect::<Vec<_>>(), [a.clone(), b.clone()], "a deleted file is dropped");
        assert_eq!(back.active, 1, "the active window is still b.py");
        assert_eq!(back.files[1], Entry { path: b, line: 10, column: 7 });

        // Another project has its own session; an unrelated folder gets the last one saved.
        assert!(load(&two).is_none());
        assert_eq!(startup(&one.join("src")).map(|s| s.files.len()), Some(2), "from a folder inside the project");
        save(&Session { files: vec![Entry { path: other.clone(), line: 1, column: 1 }], active: 0 }).unwrap();
        let unrelated = base.join("nowhere");
        std::fs::create_dir_all(&unrelated).unwrap();
        assert_eq!(startup(&unrelated).map(|s| s.files[0].path.clone()), Some(other), "the project used last");
        assert_eq!(startup(&one).map(|s| s.files.len()), Some(2), "its own project still wins");

        forget(&one);
        assert!(load(&one).is_none());
        assert!(session_file(&one).unwrap() != session_file(&two).unwrap());
        unsafe { std::env::remove_var("RUST_IDE_CONFIG_FILE") };
    }
}
