// Lesson 12: Error handling with ? and your own error type

use std::fmt;
use std::num::ParseIntError;

// Your own error type: an enum with one variant per kind of failure.
#[derive(Debug)]
enum ConfigError {
    Missing(String),
    BadNumber(ParseIntError),
    OutOfRange(i32),
}

// Display decides how the error reads to a person.
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::Missing(key) => write!(f, "missing setting '{key}'"),
            ConfigError::BadNumber(e) => write!(f, "not a number: {e}"),
            ConfigError::OutOfRange(n) => write!(f, "{n} is out of range (1 to 100)"),
        }
    }
}

impl std::error::Error for ConfigError {}

// `From` lets the ? operator convert a ParseIntError into a ConfigError by itself.
impl From<ParseIntError> for ConfigError {
    fn from(e: ParseIntError) -> Self {
        ConfigError::BadNumber(e)
    }
}

fn lookup<'a>(settings: &'a [(&'a str, &'a str)], key: &str) -> Result<&'a str, ConfigError> {
    settings
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
        .ok_or_else(|| ConfigError::Missing(key.to_string()))
}

// The ? operator: on Err, return it from this function right away; on Ok, unwrap the value.
fn read_volume(settings: &[(&str, &str)]) -> Result<i32, ConfigError> {
    let text = lookup(settings, "volume")?;
    let volume: i32 = text.trim().parse()?; // ParseIntError becomes ConfigError here
    if !(1..=100).contains(&volume) {
        return Err(ConfigError::OutOfRange(volume));
    }
    Ok(volume)
}

// Box<dyn Error> accepts any error type: convenient for main and small programs.
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let good = [("volume", " 35 ")];
    println!("volume = {}", read_volume(&good)?);

    let n: i32 = "12".parse()?;
    println!("parsed {n}");
    Ok(())
}

fn main() {
    let cases: [&[(&str, &str)]; 4] = [
        &[("volume", "50")],
        &[("name", "x")],
        &[("volume", "loud")],
        &[("volume", "500")],
    ];
    for settings in cases {
        match read_volume(settings) {
            Ok(v) => println!("ok: {v}"),
            Err(e) => println!("error: {e}"),
        }
    }

    if let Err(e) = run() {
        println!("run failed: {e}");
    }

    // A program that cannot continue can use panic!, but that is for bugs, not for expected problems.
    // Try it: add a new variant `Muted` and handle it everywhere the compiler asks.
}
