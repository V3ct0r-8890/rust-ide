// Lesson 1: Hello, world
//
// Press F5 to compile and run this file. The output appears in the Output window. Press F9 to only compile, Alt+F9 to only check for errors.
//
// Every Rust program starts at the function called `main`.

fn main() {
    // println! prints a line of text. The `!` means it is a macro, not a plain function.
    println!("Hello, world!");

    // {} is a placeholder that is replaced by the values after the string.
    println!("My name is {} and I am learning {}.", "Ada", "Rust");

    // You can also name the values inside the braces.
    let language = "Rust";
    println!("{language} is fast and safe.");

    // print! does not add a new line at the end.
    print!("No new line here, ");
    println!("but now there is.");

    // Lines that start with // are comments: the compiler ignores them.

    // Try it: change the text, press F5 again, and see the new output.
    // Try it: delete a semicolon and press Alt+F9 to see what an error message looks like.
}
