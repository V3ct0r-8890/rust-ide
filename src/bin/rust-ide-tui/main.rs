//! rust-ide-tui: the Turbo Pascal-style IDE in a terminal.
//!
//! Shares the build runner, language servers, debugger, search and syntax lexer with the
//! window version. Programs, the debugger and the language servers run on threads; their
//! events come back to the main loop through a channel.

mod app;
mod buffer;
mod clipboard;
mod live;
mod markdown;
mod ui;
mod vimkeys;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use iced::futures::Stream;
use live::Background;

use ratatui::DefaultTerminal;
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use rust_ide::runner::{self, RunTarget};

use app::{App, Effect};

const USAGE: &str = "\
Usage: rust-ide-tui [FILE...]

Turbo Pascal-style IDE for Rust, Python and C#, in the terminal.
Press F1 inside for the keyboard shortcuts.";

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("rust-ide-tui {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let files: Vec<PathBuf> = args.iter().filter(|a| !a.starts_with("--")).map(PathBuf::from).collect();

    let mouse = !args.iter().any(|a| a == "--no-mouse");
    let mut terminal = ratatui::init();
    // Turbo Pascal's block cursor.
    let _ = execute!(io::stdout(), SetCursorStyle::SteadyBlock);
    if mouse {
        let _ = execute!(io::stdout(), EnableMouseCapture);
    }
    // Pastes from the terminal arrive as one event. (Windows consoles send the text as
    // keys, and their clipboard is read directly.)
    #[cfg(not(windows))]
    let _ = execute!(io::stdout(), ratatui::crossterm::event::EnableBracketedPaste);
    let mut app = App::new(&files);
    app.check_before = rust_ide::platform::read_setting("check_before").as_deref() == Some("true");
    app.remember_dir = true;
    app.line_numbers = rust_ide::platform::read_setting("line_numbers").as_deref() == Some("true");
    app.two_line_status = rust_ide::platform::read_setting("status_lines").as_deref() == Some("2");
    app::set_theme(rust_ide::platform::read_setting("theme").as_deref().unwrap_or("turbo"));
    if rust_ide::platform::read_setting("vim").as_deref() == Some("true") {
        app.vim = Some(rust_ide::vim::Vim::default());
    }
    app.window_style = app::WindowStyle::from_setting(rust_ide::platform::read_setting("window_style").as_deref());
    app.preview_mode = app::PreviewMode::from_setting(rust_ide::platform::read_setting("preview_mode").as_deref());
    app.check_at_startup = rust_ide::platform::read_setting("check_dependencies").as_deref() != Some("false");
    // Started without a file: bring back the last session, else ask which language to use.
    if files.is_empty() {
        let session = (rust_ide::platform::read_setting("restore_session").as_deref() != Some("false"))
            .then(|| std::env::current_dir().ok().and_then(|cwd| rust_ide::session::startup(&cwd)))
            .flatten();
        match session {
            Some(session) => app.restore_session(&session),
            None => app.ask_language(true),
        }
    }
    let result = run(&mut terminal, app, mouse);
    let _ = execute!(io::stdout(), SetCursorStyle::DefaultUserShape, DisableMouseCapture);
    #[cfg(not(windows))]
    let _ = execute!(io::stdout(), ratatui::crossterm::event::DisableBracketedPaste);
    ratatui::restore();
    result
}

/// Runs a background stream on a thread of its own, sending its events to the main loop.
/// The stream is made on that thread, so it need not be `Send`.
fn spawn_stream<S, T>(tx: &mpsc::Sender<Background>, make: impl FnOnce() -> S + Send + 'static, wrap: impl Fn(T) -> Background + Send + 'static)
where
    S: Stream<Item = T> + 'static,
    T: 'static,
{
    let tx = tx.clone();
    std::thread::spawn(move || {
        for item in iced::futures::executor::block_on_stream(Box::pin(make())) {
            if tx.send(wrap(item)).is_err() {
                break;
            }
        }
    });
}

fn run(terminal: &mut DefaultTerminal, mut app: App, mouse: bool) -> io::Result<()> {
    let (tx, rx) = mpsc::channel::<Background>();
    spawn_stream(&tx, rust_ide::lsp::connect, Background::Lsp);
    if app.check_at_startup {
        let deps_tx = tx.clone();
        std::thread::spawn(move || {
            let _ = deps_tx.send(Background::Deps(rust_ide::deps::check(rust_ide::platform::Os::detect())));
        });
    }
    let mut dirty = true;
    loop {
        let size = terminal.size()?;
        if dirty {
            app.sync();
            let area = ui::editor_area(&app, ratatui::layout::Rect::new(0, 0, size.width, size.height));
            if app.buffer().preview_of.is_none() {
                app.buffer_mut().scroll_into_view(area.height as usize, area.width as usize);
            }
            terminal.draw(|frame| ui::draw(frame, &app))?;
            dirty = false;
        }

        // Events of programs, the debugger and the language servers.
        let mut effect = Effect::None;
        while let Ok(event) = rx.try_recv() {
            dirty = true;
            effect = app.background(event);
            if effect != Effect::None {
                break;
            }
        }
        if effect == Effect::None {
            if !event::poll(Duration::from_millis(40))? {
                continue;
            }
            dirty = true;
            let raw = event::read()?;
        if let Some(log) = std::env::var_os("RUST_IDE_TUI_KEYLOG") {
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)
                .and_then(|mut f| writeln!(f, "{raw:?}"));
        }
            effect = match raw {
                // Windows reports key releases too.
                Event::Key(key) if key.kind == KeyEventKind::Press => app.handle(key),
                Event::Mouse(event) => app.mouse(event, ratatui::layout::Rect::new(0, 0, size.width, size.height)),
                Event::Paste(text) => {
                    app.paste_event(&text);
                    Effect::None
                }
                _ => continue,
            };
        }
        loop {
            effect = match effect {
                Effect::None => break,
                Effect::Quit => {
                    app.stop_all();
                    app.save_session();
                    return Ok(());
                }
                Effect::Build { path, after } => {
                    // Show "Compiling..." before the (blocking) build.
                    terminal.draw(|frame| ui::draw(frame, &app))?;
                    let result = runner::build(&path);
                    app.build_finished(result, after)
                }
                Effect::Check { path } => {
                    terminal.draw(|frame| ui::draw(frame, &app))?;
                    let result = runner::check(&path);
                    app.check_finished(result)
                }
                Effect::Copy(text) => {
                    clipboard::write(&text);
                    Effect::None
                }
                Effect::Paste => {
                    let text = clipboard::read().filter(|t| !t.is_empty()).unwrap_or_else(|| app.clipboard.clone());
                    app.paste(&text);
                    Effect::None
                }
                Effect::CheckDeps { startup } => {
                    terminal.draw(|frame| ui::draw(frame, &app))?;
                    app.deps_checked(rust_ide::deps::check(rust_ide::platform::Os::detect()), startup);
                    Effect::None
                }
                Effect::InstallDep { name, command } => {
                    terminal.draw(|frame| ui::draw(frame, &app))?;
                    let result = rust_ide::deps::install(&command);
                    app.dep_installed(name, result)
                }
                Effect::PythonSetup(dir) => {
                    terminal.draw(|frame| ui::draw(frame, &app))?;
                    app.status = rust_ide::env::python_setup(&dir).unwrap_or_else(|e| e);
                    Effect::None
                }
                Effect::StartRun { target, run_id } => {
                    spawn_stream(&tx, move || rust_ide::output::run(target), move |e| Background::Output(run_id, e));
                    Effect::None
                }
                Effect::StartDebug { launch, run_id } => {
                    spawn_stream(&tx, move || rust_ide::dap::run(launch), move |e| Background::Debug(run_id, e));
                    Effect::None
                }
                Effect::RunHere(target) => {
                    let status = user_screen(terminal, &target, mouse)?;
                    app.status = status;
                    Effect::None
                }
            };
        }
    }
}

/// Leaves the IDE screen to run a program on the terminal's own screen (when no terminal
/// window of its own can be opened), then waits for a key and comes back.
fn user_screen(terminal: &mut DefaultTerminal, target: &RunTarget, mouse: bool) -> io::Result<String> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, SetCursorStyle::DefaultUserShape, DisableMouseCapture)?;

    let status;
    {
        if target.gui {
            status = match Command::new(&target.program).envs(rust_ide::env::effective(Some(&target.cwd))).args(&target.args).current_dir(&target.cwd).spawn() {
                Ok(_) => "Running (GUI window)".into(),
                Err(e) => format!("Cannot run: {e}"),
            };
        } else {
            println!();
            let result = Command::new(&target.program).envs(rust_ide::env::effective(Some(&target.cwd))).args(&target.args).current_dir(&target.cwd).status();
            status = match result {
                Ok(exit) => match exit.code() {
                    Some(code) => format!("Program finished, exit code {code} (Alt+F5 shows its output)"),
                    None => "Program stopped".into(),
                },
                Err(e) => format!("Cannot run {}: {e}", target.program.display()),
            };
        }
    }
    if !target.gui {
        print!("\n[{status}]  Press any key to return to the IDE...");
        io::stdout().flush()?;
        enable_raw_mode()?;
        loop {
            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                break;
            }
        }
        disable_raw_mode()?;
    }

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, SetCursorStyle::SteadyBlock)?;
    if mouse {
        execute!(io::stdout(), EnableMouseCapture)?;
    }
    terminal.clear()?;
    Ok(status)
}
