//! rust-ide-tui: the Turbo Pascal-style IDE in a terminal.
//!
//! Shares the build runner, search and syntax lexer with the GUI. F5 runs
//! the program right in the terminal, like Turbo Pascal's user screen.

mod app;
mod buffer;
mod ui;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use ratatui::DefaultTerminal;
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use rust_ide::runner::{self, RunTarget};

use app::{App, Effect};

const USAGE: &str = "\
Usage: rust-ide-tui [FILE...]

Turbo Pascal-style IDE for Rust and Python, in the terminal.
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
    let mut app = App::new(&files);
    app.check_before = rust_ide::platform::read_setting("check_before").as_deref() == Some("true");
    // Started without a file: ask which language to use.
    if files.is_empty() {
        app.ask_language(true);
    }
    let result = run(&mut terminal, app, mouse);
    let _ = execute!(io::stdout(), SetCursorStyle::DefaultUserShape, DisableMouseCapture);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, mut app: App, mouse: bool) -> io::Result<()> {
    loop {
        let size = terminal.size()?;
        let area = ui::editor_area(&app, ratatui::layout::Rect::new(0, 0, size.width, size.height));
        app.buffer_mut().scroll_into_view(area.height as usize, area.width as usize);
        terminal.draw(|frame| ui::draw(frame, &app))?;

        let raw = event::read()?;
        if let Some(log) = std::env::var_os("RUST_IDE_TUI_KEYLOG") {
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)
                .and_then(|mut f| writeln!(f, "{raw:?}"));
        }
        let mut effect = match raw {
            // Windows reports key releases too.
            Event::Key(key) if key.kind == KeyEventKind::Press => app.handle(key),
            Event::Mouse(event) => app.mouse(event, ratatui::layout::Rect::new(0, 0, size.width, size.height)),
            _ => continue,
        };
        loop {
            effect = match effect {
                Effect::None => break,
                Effect::Quit => return Ok(()),
                Effect::Build { path, run } => {
                    // Show "Compiling..." before the (blocking) build.
                    terminal.draw(|frame| ui::draw(frame, &app))?;
                    let result = runner::build(&path);
                    app.build_finished(result, run)
                }
                Effect::Check { path } => {
                    terminal.draw(|frame| ui::draw(frame, &app))?;
                    let result = runner::check(&path);
                    app.check_finished(result)
                }
                Effect::Run(target) => {
                    let status = user_screen(terminal, Some(&target), mouse)?;
                    app.status = status;
                    Effect::None
                }
                Effect::UserScreen => {
                    user_screen(terminal, None, mouse)?;
                    Effect::None
                }
            };
        }
    }
}

/// Leaves the IDE screen to run a program (or just show the terminal's own
/// screen), then waits for a key and comes back.
fn user_screen(terminal: &mut DefaultTerminal, target: Option<&RunTarget>, mouse: bool) -> io::Result<String> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, SetCursorStyle::DefaultUserShape, DisableMouseCapture)?;

    let mut status = String::from("User screen");
    if let Some(target) = target {
        if target.gui {
            status = match Command::new(&target.program).args(&target.args).current_dir(&target.cwd).spawn() {
                Ok(_) => "Running (GUI window)".into(),
                Err(e) => format!("Cannot run: {e}"),
            };
        } else {
            println!();
            let result = Command::new(&target.program).args(&target.args).current_dir(&target.cwd).status();
            status = match result {
                Ok(exit) => match exit.code() {
                    Some(code) => format!("Program finished, exit code {code} (Alt+F5 shows its output)"),
                    None => "Program stopped".into(),
                },
                Err(e) => format!("Cannot run {}: {e}", target.program.display()),
            };
        }
    }
    if target.is_none_or(|t| !t.gui) {
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
