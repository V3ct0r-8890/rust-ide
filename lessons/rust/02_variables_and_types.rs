// Lesson 2: Variables and basic types
//
// `let` creates a variable. Variables are immutable (cannot change) unless you add `mut`.

fn main() {
    let age = 30; // Rust works out the type (i32) by itself
    println!("age = {age}");

    // age = 31;  // <- remove the // and compile: you cannot assign twice to an immutable variable

    let mut score = 10; // `mut` makes it changeable
    score = score + 5;
    score += 1; // shorthand for score = score + 1
    println!("score = {score}");

    // You can write the type yourself after a colon.
    let big: i64 = 9_000_000_000; // underscores make long numbers easier to read
    let small: u8 = 255; // u8 holds 0..=255
    let ratio: f64 = 0.75;
    let letter: char = 'R'; // one character, in single quotes
    let ready: bool = true;
    println!("{big} {small} {ratio} {letter} {ready}");

    // Shadowing: a new `let` with the same name hides the old one (the type may change).
    let spaces = "   ";
    let spaces = spaces.len();
    println!("there are {spaces} spaces");

    // Constants are always immutable, need a type, and are named in UPPER_CASE.
    const MAX_PLAYERS: u32 = 4;
    println!("max players: {MAX_PLAYERS}");

    // Tuples group a few values of different types.
    let point = (3, 4.5, "north");
    let (x, y, direction) = point; // "destructuring"
    println!("{x} {y} {direction} / first item: {}", point.0);

    // Try it: make `age` mutable and change it.
    // Try it: set `small` to 256 and see the compiler stop you.
}
