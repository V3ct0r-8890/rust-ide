// Lesson 18: Lifetimes
//
// A lifetime says how long a reference is valid. Most of the time Rust works them out itself.
// You write them (like 'a) when a function returns a reference and the compiler cannot tell
// which of the inputs it came from.

// Without 'a the compiler asks: does the result borrow from `x` or from `y`?
// 'a says: the result lives no longer than BOTH inputs.
fn longer<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() >= y.len() { x } else { y }
}

// Only one input is returned, so only that one needs the lifetime.
fn first_part<'a>(text: &'a str, _separator: &str) -> &'a str {
    text.split(',').next().unwrap_or("")
}

// A struct that holds a reference needs a lifetime too: it cannot outlive the text it borrows.
struct Excerpt<'a> {
    text: &'a str,
}

impl<'a> Excerpt<'a> {
    fn first_sentence(&self) -> &'a str {
        self.text.split('.').next().unwrap_or("")
    }
}

// 'static means "valid for the whole program": string literals have it.
fn greeting() -> &'static str {
    "hello"
}

fn main() {
    let a = String::from("a long sentence");
    let b = String::from("short");
    println!("{}", longer(&a, &b));

    // The result is only valid while both inputs are. This block shows why:
    let outer = String::from("outer");
    let result;
    {
        let inner = String::from("inner text");
        result = longer(&outer, &inner).len(); // fine: we only keep a number
        // If we kept the &str itself and used it after this block, `inner` would be gone:
        // that is exactly what lifetimes prevent.
    }
    println!("{result}");

    let novel = String::from("Call me Ishmael. Some years ago...");
    let excerpt = Excerpt { text: &novel };
    println!("{}", excerpt.first_sentence());
    println!("{}", first_part("red,green", ","));
    println!("{}", greeting());

    // Rules of thumb:
    //  - Return an owned String (not a &str) if lifetimes get confusing; cloning is fine to start.
    //  - Prefer owned fields (String) in structs until you need to borrow.

    // Try it: change `longer` to return String (use .to_string()) and remove all the 'a.
}
