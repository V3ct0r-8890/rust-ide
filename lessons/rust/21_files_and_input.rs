// Lesson 21: Reading input and working with files
//
// Run this with F5. When the program asks for a name, type it in the input line of the
// Output window and press Enter (or just press Enter to use the default).

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

fn ask(prompt: &str) -> String {
    print!("{prompt}");
    io::stdout().flush().unwrap(); // make sure the prompt appears before we wait
    let mut line = String::new();
    // read_line returns Ok(0) at the end of the input, so this also works without a keyboard.
    let _ = io::stdin().lock().read_line(&mut line);
    line.trim().to_string()
}

fn main() -> io::Result<()> {
    let name = ask("What is your name? ");
    let name = if name.is_empty() { "friend".to_string() } else { name };
    println!("Hello, {name}!");

    // A folder for the files of this lesson, inside the system's temporary folder.
    let dir: PathBuf = std::env::temp_dir().join("rust-ide-lesson21");
    fs::create_dir_all(&dir)?;
    let file = dir.join("notes.txt");

    // Write a whole file at once.
    fs::write(&file, "first line\nsecond line\n")?;

    // Append to it.
    let mut handle = fs::OpenOptions::new().append(true).open(&file)?;
    writeln!(handle, "third line for {name}")?;
    drop(handle);

    // Read the whole file into a String.
    let text = fs::read_to_string(&file)?;
    println!("--- {} ---\n{text}---", file.display());

    // Read line by line (better for big files).
    let reader = io::BufReader::new(fs::File::open(&file)?);
    for (number, line) in reader.lines().enumerate() {
        println!("{:>2}: {}", number + 1, line?);
    }

    // Paths and file information.
    println!("exists: {}", file.exists());
    println!("size: {} bytes", fs::metadata(&file)?.len());
    println!("extension: {:?}", file.extension());
    println!("file name: {:?}", file.file_name());

    // List a folder.
    fs::write(dir.join("other.txt"), "x")?;
    let mut names: Vec<String> = fs::read_dir(&dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    println!("{names:?}");

    // Errors are normal with files: a missing file is an Err, not a crash.
    match fs::read_to_string(dir.join("missing.txt")) {
        Ok(_) => println!("found?"),
        Err(e) => println!("cannot read: {e}"),
    }

    // Clean up.
    fs::remove_dir_all(&dir)?;
    println!("cleaned up");
    Ok(()) // main can return a Result; an Err prints the error and exits with code 1

    // Try it: ask for two numbers and print their sum (parse them as in lesson 11).
}
