// Lesson 15: Closures and iterators

fn make_adder(amount: i32) -> impl Fn(i32) -> i32 {
    move |x| x + amount // `move` gives the closure its own copy of `amount`
}

fn apply<F: Fn(i32) -> i32>(f: F, value: i32) -> i32 {
    f(value)
}

fn main() {
    // A closure is a small anonymous function: |parameters| body
    let square = |x: i32| x * x;
    println!("{}", square(7));

    // Closures can use variables from the place where they were written.
    let factor = 3;
    let times_factor = |x| x * factor;
    println!("{}", times_factor(5));
    println!("{}", apply(times_factor, 10));

    let add5 = make_adder(5);
    println!("{}", add5(10));

    // Iterators produce items one by one. Adapters (map, filter...) are lazy: nothing happens
    // until a consumer (collect, sum, for...) asks for the items.
    let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    let squares: Vec<i32> = numbers.iter().map(|n| n * n).collect();
    println!("{squares:?}");

    let evens: Vec<&i32> = numbers.iter().filter(|n| *n % 2 == 0).collect();
    println!("{evens:?}");

    // Chains read like a sentence: "the sum of the squares of the odd numbers".
    let total: i32 = numbers.iter().filter(|n| *n % 2 == 1).map(|n| n * n).sum();
    println!("{total}");

    println!("{:?}", numbers.iter().max());
    println!("{}", numbers.iter().any(|n| *n > 9));
    println!("{}", numbers.iter().all(|n| *n > 0));
    println!("{:?}", numbers.iter().position(|n| *n == 4));
    println!("{:?}", numbers.iter().find(|n| **n > 6));
    println!("{}", numbers.iter().count());

    // take, skip, zip, enumerate, rev, chain
    let firsts: Vec<_> = numbers.iter().take(3).collect();
    let rest: Vec<_> = numbers.iter().skip(8).collect();
    println!("{firsts:?} {rest:?}");
    let names = ["a", "b", "c"];
    for (name, n) in names.iter().zip(&numbers) {
        print!("{name}{n} ");
    }
    println!();
    let backwards: Vec<_> = (1..=5).rev().collect();
    println!("{backwards:?}");

    // fold: combine all items into one value.
    let product = (1..=5).fold(1, |acc, n| acc * n);
    println!("{product}");

    // collect can build other collections, even a String or a Result.
    let word: String = vec!['r', 'u', 's', 't'].into_iter().collect();
    println!("{word}");
    let parsed: Result<Vec<i32>, _> = "1 2 3".split(' ').map(|s| s.parse::<i32>()).collect();
    println!("{parsed:?}");

    // Sorting with a closure.
    let mut people = vec![("Ada", 36), ("Linus", 28), ("Grace", 45)];
    people.sort_by_key(|p| p.1);
    println!("{people:?}");

    // Try it: use filter and map to get the lengths of the words longer than 3 letters
    // in "the quick brown fox jumps".
}
