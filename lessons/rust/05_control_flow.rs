// Lesson 5: Control flow: if, loops and match

fn main() {
    // if / else if / else. The condition must be a bool (no 0 or 1 tricks).
    let temperature = 22;
    if temperature > 30 {
        println!("hot");
    } else if temperature > 15 {
        println!("pleasant");
    } else {
        println!("cold");
    }

    // if is an expression: it gives back a value.
    let label = if temperature % 2 == 0 { "even" } else { "odd" };
    println!("{label}");

    // Combine conditions with && (and), || (or), ! (not).
    let weekend = false;
    if !weekend && temperature > 20 {
        println!("work day, nice weather");
    }

    // loop repeats forever until you break. It can return a value.
    let mut counter = 0;
    let result = loop {
        counter += 1;
        if counter == 5 {
            break counter * 10;
        }
    };
    println!("loop gave {result}");

    // while repeats as long as the condition is true.
    let mut n = 3;
    while n > 0 {
        println!("countdown {n}");
        n -= 1;
    }

    // for goes through a range or a collection. 1..4 is 1,2,3; 1..=4 is 1,2,3,4.
    for i in 1..=3 {
        println!("i = {i}");
    }
    for letter in ['a', 'b', 'c'] {
        println!("letter {letter}");
    }
    for i in (0..10).step_by(5) {
        println!("step {i}");
    }

    // continue skips to the next round, break leaves the loop.
    for i in 1..=10 {
        if i % 2 == 0 {
            continue;
        }
        if i > 7 {
            break;
        }
        println!("odd {i}");
    }

    // Labels let you break out of an outer loop.
    'outer: for x in 1..=3 {
        for y in 1..=3 {
            if x * y == 4 {
                println!("found {x} * {y}");
                break 'outer;
            }
        }
    }

    // match compares a value against patterns. It must cover every case.
    let number = 3;
    match number {
        1 => println!("one"),
        2 | 3 => println!("two or three"),
        4..=9 => println!("four to nine"),
        _ => println!("something else"), // _ matches everything left
    }

    // Try it: print the numbers 1 to 20, but "Fizz" for multiples of 3, "Buzz" for 5,
    // and "FizzBuzz" for both.
}
