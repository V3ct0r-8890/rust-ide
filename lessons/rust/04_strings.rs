// Lesson 4: Strings
//
// Rust has two main string types:
//   &str   a borrowed piece of text (like "hello" written in the code), it cannot grow
//   String an owned, growable text stored on the heap

fn main() {
    let literal: &str = "hello";
    let mut owned: String = String::from("hello");
    owned.push_str(", world"); // add text at the end
    owned.push('!'); // add one character
    println!("{literal} / {owned}");

    // Create a String in different ways.
    let a = "abc".to_string();
    let b = String::new();
    let c = format!("{a}-{}", 42); // format! builds a String like println! prints
    println!("{a:?} {b:?} {c}"); // {:?} shows quotes: handy for debugging

    // Useful methods.
    let text = "  The quick brown fox  ";
    println!("[{}]", text.trim());
    println!("{}", text.to_uppercase());
    println!("{}", text.contains("quick"));
    println!("{}", text.replace("quick", "slow"));
    println!("{}", text.trim().len()); // length in BYTES

    // Splitting and joining.
    let csv = "red,green,blue";
    let parts: Vec<&str> = csv.split(',').collect();
    println!("{parts:?}");
    println!("{}", parts.join(" + "));
    for word in "one two  three".split_whitespace() {
        println!("word: {word}");
    }

    // Characters. A Rust string is UTF-8, so one character can use several bytes.
    let thai = "สวัสดี";
    println!("{} bytes, {} chars", thai.len(), thai.chars().count());
    for (i, ch) in "héllo".chars().enumerate() {
        println!("{i}: {ch}");
    }

    // Slicing uses byte positions and must fall on character boundaries.
    let hello = "hello world";
    println!("{}", &hello[0..5]);
    println!("{}", &hello[6..]);

    // Parsing text into numbers returns a Result (see lesson 11).
    let n: i32 = "42".parse().unwrap();
    println!("{}", n + 1);
    println!("{:?}", "abc".parse::<i32>().is_err());

    // Comparing and starting/ending.
    println!("{}", "apple" < "banana");
    println!("{} {}", hello.starts_with("hello"), hello.ends_with("x"));

    // Try it: reverse a string with text.chars().rev().collect::<String>().
}
