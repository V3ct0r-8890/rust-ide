// Lesson 7: Ownership
//
// This is the idea that makes Rust different. Three rules:
//   1. Every value has exactly one owner (a variable).
//   2. When the owner goes out of scope, the value is dropped (its memory is freed).
//   3. Assigning or passing a value MOVES the ownership (for types that own heap data).

fn takes_ownership(text: String) {
    println!("inside: {text}");
} // `text` is dropped here and its memory is freed

fn gives_back(text: String) -> String {
    println!("borrowed for a moment: {text}");
    text // return it, so the caller owns it again
}

fn main() {
    // Move: after `b = a`, the string belongs to `b`.
    let a = String::from("hello");
    let b = a;
    // println!("{a}");   // <- error: `a` was moved. Remove the // to see the message.
    println!("{b}");

    // Copy: simple types that live on the stack are copied, not moved.
    let x = 5;
    let y = x;
    println!("{x} {y}"); // both still usable

    // Passing to a function moves the value too.
    let s = String::from("world");
    takes_ownership(s);
    // println!("{s}");   // <- error: moved into the function

    // Give it back to keep using it...
    let t = gives_back(String::from("again"));
    println!("{t}");

    // ...or clone it to make a real copy (costs time and memory).
    let original = String::from("copy me");
    let copy = original.clone();
    println!("{original} / {copy}");

    // Scope: values are dropped at the closing brace, in reverse order.
    {
        let inner = String::from("short lived");
        println!("{inner}");
    }
    // `inner` no longer exists here.

    // Why does Rust do this? No garbage collector, no manual free(), and no use-after-free
    // or double-free bugs: the compiler checks all of it before the program runs.
    // The next lesson shows how to use a value without taking it: borrowing.

    // Try it: remove the // in front of one of the commented println! lines and read the error.
}
