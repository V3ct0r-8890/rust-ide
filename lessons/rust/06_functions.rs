// Lesson 6: Functions

// A function has a name, parameters with types, and an optional return type after ->.
fn add(a: i32, b: i32) -> i32 {
    a + b // no semicolon: the last expression is the return value
}

fn greet(name: &str) {
    println!("Hello, {name}!"); // no -> means it returns nothing (the "unit" type ())
}

// `return` leaves early.
fn absolute(n: i32) -> i32 {
    if n < 0 {
        return -n;
    }
    n
}

// Return several values with a tuple.
fn min_max(numbers: &[i32]) -> (i32, i32) {
    let mut min = numbers[0];
    let mut max = numbers[0];
    for &n in numbers {
        if n < min {
            min = n;
        }
        if n > max {
            max = n;
        }
    }
    (min, max)
}

// A function can call itself: recursion.
fn factorial(n: u64) -> u64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}

// Functions can take other functions as values.
fn apply_twice(f: fn(i32) -> i32, value: i32) -> i32 {
    f(f(value))
}

fn double(x: i32) -> i32 {
    x * 2
}

fn main() {
    println!("{}", add(2, 3));
    greet("Ada");
    println!("{}", absolute(-7));

    let (lo, hi) = min_max(&[4, 9, -2, 7]);
    println!("min {lo}, max {hi}");

    println!("5! = {}", factorial(5));
    println!("{}", apply_twice(double, 3));

    // A block { } is an expression too.
    let y = {
        let x = 3;
        x * x + 1
    };
    println!("{y}");

    // Try it: write fn is_prime(n: u32) -> bool and test it with the numbers 1 to 30.
}
