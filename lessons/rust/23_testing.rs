// Lesson 23: Testing
//
// Rust has testing built in. A test is a function marked #[test] that panics when something
// is wrong. Tests live next to the code in a module that only exists when testing.
//
// How to run the tests:
//   - In a Cargo project: `cargo test` in a terminal.
//   - For this single file: `rustc --test --edition 2024 23_testing.rs` and run the result.
// F5 in the IDE runs main() below, which shows the same functions at work.

pub fn is_even(n: i32) -> bool {
    n % 2 == 0
}

pub fn clamp(value: i32, low: i32, high: i32) -> i32 {
    if value < low {
        low
    } else if value > high {
        high
    } else {
        value
    }
}

pub fn divide(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 { Err("division by zero".into()) } else { Ok(a / b) }
}

pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn main() {
    println!("{} {}", is_even(4), is_even(7));
    println!("{} {} {}", clamp(5, 0, 10), clamp(-3, 0, 10), clamp(99, 0, 10));
    println!("{:?} {:?}", divide(9, 3), divide(1, 0));
    println!("{}", word_count("the quick  brown fox"));
}

#[cfg(test)] // compiled only for tests
mod tests {
    use super::*; // bring in the functions above

    #[test]
    fn even_numbers() {
        assert!(is_even(2));
        assert!(!is_even(3));
        assert!(is_even(0));
        assert!(is_even(-4));
    }

    #[test]
    fn clamp_keeps_value_inside() {
        assert_eq!(clamp(5, 0, 10), 5);
        assert_eq!(clamp(-1, 0, 10), 0);
        assert_eq!(clamp(11, 0, 10), 10);
    }

    #[test]
    fn divide_reports_errors() {
        assert_eq!(divide(10, 2), Ok(5));
        assert_eq!(divide(1, 0), Err("division by zero".to_string()));
    }

    #[test]
    fn counting_words_ignores_extra_spaces() {
        assert_eq!(word_count(""), 0);
        assert_eq!(word_count("  a   b "), 2);
    }

    // A test can expect a panic.
    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn indexing_past_the_end_panics() {
        let items = vec![1, 2, 3];
        let position = items.len() + 2;
        let _ = items[position];
    }

    // A test can return a Result, so the ? operator works inside it.
    #[test]
    fn parsing_works() -> Result<(), std::num::ParseIntError> {
        let n: i32 = "42".parse()?;
        assert_eq!(n, 42);
        Ok(())
    }
}

// Tips:
//  - assert_eq!(left, right) prints both values when they differ.
//  - One test per behavior; give tests names that read like sentences.
//  - Try it: add a test for a new function `fn double(n: i32) -> i32`, first make it fail, then pass.
