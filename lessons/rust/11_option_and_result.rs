// Lesson 11: Option and Result
//
// Rust has no null and no exceptions. Instead:
//   Option<T>     either Some(value) or None             "there might be nothing"
//   Result<T, E>  either Ok(value) or Err(error)         "this might fail"

fn find_index(items: &[&str], wanted: &str) -> Option<usize> {
    for (i, item) in items.iter().enumerate() {
        if *item == wanted {
            return Some(i);
        }
    }
    None
}

fn divide(a: f64, b: f64) -> Result<f64, String> {
    if b == 0.0 {
        Err(String::from("cannot divide by zero"))
    } else {
        Ok(a / b)
    }
}

fn main() {
    let fruits = ["apple", "banana", "cherry"];

    // Handle both cases with match.
    match find_index(&fruits, "banana") {
        Some(i) => println!("banana is at {i}"),
        None => println!("not found"),
    }

    // if let when you only care about one case.
    if let Some(i) = find_index(&fruits, "cherry") {
        println!("cherry is at {i}");
    }

    // Handy helpers.
    let missing = find_index(&fruits, "kiwi");
    println!("{}", missing.unwrap_or(0)); // a default value
    println!("{}", missing.is_none());
    let doubled = find_index(&fruits, "banana").map(|i| i * 2); // change the value inside
    println!("{doubled:?}");

    // unwrap() and expect() take the value or crash the program. Use them only in tests,
    // quick experiments, or when a failure is truly impossible.
    let first = fruits.first().expect("the list is not empty");
    println!("{first}");

    // Results work the same way.
    match divide(10.0, 4.0) {
        Ok(value) => println!("10 / 4 = {value}"),
        Err(message) => println!("error: {message}"),
    }
    println!("{:?}", divide(1.0, 0.0));

    // Parsing text gives a Result.
    for text in ["42", "forty-two", "-7"] {
        match text.parse::<i32>() {
            Ok(n) => println!("{text} -> number {n}"),
            Err(e) => println!("{text} -> {e}"),
        }
    }

    // Many standard methods return an Option.
    let numbers = vec![3, 8, 1];
    println!("{:?} {:?}", numbers.iter().max(), numbers.get(10));

    // Try it: write fn safe_sqrt(x: f64) -> Option<f64> that gives None for negative numbers.
}
