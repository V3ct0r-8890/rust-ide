// Lesson 17: Generics
//
// Generics let one piece of code work for many types. <T> is a placeholder for "some type".

use std::fmt::Display;

// A generic function: T must be comparable and copyable.
fn largest<T: PartialOrd + Copy>(items: &[T]) -> T {
    let mut best = items[0];
    for &item in items {
        if item > best {
            best = item;
        }
    }
    best
}

// A generic struct.
#[derive(Debug)]
struct Pair<T> {
    first: T,
    second: T,
}

impl<T: PartialOrd + Display> Pair<T> {
    fn show_larger(&self) {
        if self.first >= self.second {
            println!("larger: {}", self.first);
        } else {
            println!("larger: {}", self.second);
        }
    }
}

// Two type parameters.
struct Labeled<K, V> {
    key: K,
    value: V,
}

// `where` clauses keep long bounds readable.
fn print_all<T>(items: &[T])
where
    T: Display,
{
    let texts: Vec<String> = items.iter().map(|i| i.to_string()).collect();
    println!("[{}]", texts.join(", "));
}

// A small generic container of your own.
struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    fn new() -> Self {
        Stack { items: Vec::new() }
    }
    fn push(&mut self, item: T) {
        self.items.push(item);
    }
    fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }
    fn peek(&self) -> Option<&T> {
        self.items.last()
    }
    fn len(&self) -> usize {
        self.items.len()
    }
    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

fn main() {
    println!("{}", largest(&[3, 9, 2]));
    println!("{}", largest(&[1.5, 0.2]));
    println!("{}", largest(&['x', 'b']));

    let pair = Pair { first: 10, second: 4 };
    pair.show_larger();
    Pair { first: "apple", second: "pear" }.show_larger();
    println!("{pair:?}");

    let entry = Labeled { key: "age", value: 36 };
    println!("{} = {}", entry.key, entry.value);

    print_all(&[1, 2, 3]);
    print_all(&["a", "b"]);

    // Option and Vec from earlier lessons are generic types: Option<T>, Vec<T>, Result<T, E>.
    let mut stack = Stack::new();
    stack.push("one");
    stack.push("two");
    println!("{:?} {}", stack.peek(), stack.len());
    println!("{:?}", stack.pop());
    println!("{:?} {:?}", stack.pop(), stack.pop());
    println!("{}", stack.is_empty());

    // Rust makes a specialised copy for every type you use ("monomorphization"),
    // so generics cost nothing at run time.

    // Try it: add `fn smallest` or a `Stack::sum` that works for numbers only.
}
