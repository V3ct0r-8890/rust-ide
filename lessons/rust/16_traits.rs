// Lesson 16: Traits
//
// A trait describes behavior that different types can share, like an interface in other languages.

trait Animal {
    fn name(&self) -> String;
    fn sound(&self) -> String;

    // A default method: types may use it or replace it.
    fn describe(&self) -> String {
        format!("{} says {}", self.name(), self.sound())
    }
}

struct Dog;
struct Cat {
    lives: u8,
}

impl Animal for Dog {
    fn name(&self) -> String {
        "Dog".to_string()
    }
    fn sound(&self) -> String {
        "woof".to_string()
    }
}

impl Animal for Cat {
    fn name(&self) -> String {
        format!("Cat with {} lives", self.lives)
    }
    fn sound(&self) -> String {
        "meow".to_string()
    }
    fn describe(&self) -> String {
        format!("{} purrs", self.name())
    }
}

// Trait bound: accept ANY type that implements Animal (resolved at compile time, fast).
fn introduce(animal: &impl Animal) {
    println!("{}", animal.describe());
}

// Trait objects: a list of different types behind one trait (resolved at run time).
fn chorus(animals: &[Box<dyn Animal>]) {
    for a in animals {
        println!("- {}", a.sound());
    }
}

// Common standard traits can be derived automatically.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
struct Version {
    major: u32,
    minor: u32,
}

// Implement a standard trait by hand: Display controls how {} prints.
impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "v{}.{}", self.major, self.minor)
    }
}

fn main() {
    introduce(&Dog);
    introduce(&Cat { lives: 9 });

    let zoo: Vec<Box<dyn Animal>> = vec![Box::new(Dog), Box::new(Cat { lives: 7 })];
    chorus(&zoo);

    let a = Version { major: 1, minor: 4 };
    let b = Version { major: 1, minor: 10 };
    println!("{a} < {b}: {}", a < b); // PartialOrd compares field by field
    println!("{}", a == a.clone());
    println!("{:?}", Version::default());

    // Operators are traits too. For example, + is std::ops::Add.
    use std::ops::Add;
    #[derive(Debug, Clone, Copy)]
    struct Point {
        x: i32,
        y: i32,
    }
    impl Add for Point {
        type Output = Point;
        fn add(self, other: Point) -> Point {
            Point { x: self.x + other.x, y: self.y + other.y }
        }
    }
    println!("{:?}", Point { x: 1, y: 2 } + Point { x: 10, y: 20 });

    // Try it: add a Bird type and put it in the zoo.
}
