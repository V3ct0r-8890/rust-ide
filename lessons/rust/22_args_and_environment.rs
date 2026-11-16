// Lesson 22: Command-line arguments, environment variables and exit codes
//
// A Rust program can receive text from the command line. In a terminal you would run:
//     lesson22 one two three
// The IDE starts it without arguments, so try the examples in the code or run it in a terminal.
// Without arguments the program shows how to behave sensibly.

use std::env;
use std::process;

#[derive(Debug)]
struct Options {
    verbose: bool,
    count: u32,
    names: Vec<String>,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options { verbose: false, count: 1, names: Vec::new() };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-v" | "--verbose" => options.verbose = true,
            "-n" | "--count" => {
                i += 1;
                let value = args.get(i).ok_or("--count needs a number")?;
                options.count = value.parse().map_err(|_| format!("'{value}' is not a number"))?;
            }
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}")),
            name => options.names.push(name.to_string()),
        }
        i += 1;
    }
    Ok(options)
}

fn main() {
    // args() gives the program path first, then the real arguments.
    let args: Vec<String> = env::args().skip(1).collect();
    println!("arguments: {args:?}");

    // Parsing is easy to test: it takes a plain slice.
    let demo: Vec<String> = ["-v", "-n", "2", "Ada", "Linus"].iter().map(|s| s.to_string()).collect();
    println!("{:?}", parse(&demo));
    println!("{:?}", parse(&["--oops".to_string()]));
    println!("{:?}", parse(&["-n".to_string()]));

    let options = match parse(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}"); // eprintln! writes to the error stream
            process::exit(2); // a non-zero exit code tells other programs that it failed
        }
    };
    for _ in 0..options.count {
        for name in &options.names {
            println!("hello {name}");
        }
    }

    // Environment variables.
    match env::var("HOME").or_else(|_| env::var("USERPROFILE")) {
        Ok(home) => println!("home folder: {home}"),
        Err(_) => println!("no home folder set"),
    }
    let level = env::var("LESSON_LEVEL").unwrap_or_else(|_| "default".to_string());
    println!("LESSON_LEVEL = {level}");

    println!("running on {} / {}", env::consts::OS, env::consts::ARCH);
    println!("current folder: {}", env::current_dir().unwrap().display());

    // In the IDE you can set variables for your programs in Options > General environment or Project environment.
    // Try it: set LESSON_LEVEL there and run again.
}
