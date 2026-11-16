// Lesson 8: Borrowing and references
//
// A reference lets you use a value without owning it.
//   &value      a shared reference: read only, as many as you like at once
//   &mut value  a mutable reference: can change the value, but only ONE at a time
// A reference can never outlive the value it points to.

fn length(text: &String) -> usize {
    text.len() // we only read, so a shared reference is enough
}

fn shout(text: &mut String) {
    text.push_str("!!!"); // changes the caller's string
}

// Prefer &str over &String for parameters: it accepts both.
fn first_word(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or("")
}

fn main() {
    let mut greeting = String::from("hello brave world");

    // Shared borrows: the owner keeps ownership.
    let n = length(&greeting);
    println!("{greeting} has {n} bytes");

    // Several shared borrows at the same time are fine.
    let r1 = &greeting;
    let r2 = &greeting;
    println!("{r1} {r2}");

    // A mutable borrow needs `mut` on the variable and `&mut` at the call.
    shout(&mut greeting);
    println!("{greeting}");

    // The rule: either many readers OR one writer, never both at once.
    let writer = &mut greeting;
    writer.push('?');
    // let reader = &greeting;   // <- error while `writer` is still used below
    println!("{writer}");

    // Slices are references to a part of a value.
    let word = first_word(&greeting);
    println!("first word: {word}");
    let part = &greeting[0..5];
    println!("{part}");

    // Dereferencing: * follows a reference to the value. Method calls and printing do it for you.
    let mut count = 10;
    let c = &mut count;
    *c += 1;
    println!("{count}");

    // Try it: uncomment the `reader` line and read the error. Rust prevents data races at compile time.
}
