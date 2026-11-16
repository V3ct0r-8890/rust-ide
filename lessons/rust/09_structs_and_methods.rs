// Lesson 9: Structs and methods

// A struct groups related data under one name.
#[derive(Debug, Clone)] // lets us print it with {:?} and copy it with .clone()
struct Rectangle {
    width: f64,
    height: f64,
}

// `impl` adds functions that belong to the type.
impl Rectangle {
    // An associated function (no self): called as Rectangle::new(...). Often used as a constructor.
    fn new(width: f64, height: f64) -> Rectangle {
        Rectangle { width, height } // field init shorthand: same name as the parameter
    }

    fn square(side: f64) -> Rectangle {
        Rectangle::new(side, side)
    }

    // &self: a method that only reads.
    fn area(&self) -> f64 {
        self.width * self.height
    }

    fn can_hold(&self, other: &Rectangle) -> bool {
        self.width > other.width && self.height > other.height
    }

    // &mut self: a method that changes the value.
    fn scale(&mut self, factor: f64) {
        self.width *= factor;
        self.height *= factor;
    }
}

// A tuple struct: fields have positions instead of names.
struct Color(u8, u8, u8);

// A unit struct has no data. It is used with traits (lesson 16).
struct Marker;

#[derive(Debug)]
struct Player {
    name: String,
    score: u32,
    active: bool,
}

fn main() {
    let mut rect = Rectangle::new(3.0, 4.0);
    println!("{rect:?} area = {}", rect.area());

    rect.scale(2.0);
    println!("after scale: {rect:?}");

    let small = Rectangle::square(2.0);
    println!("can hold small: {}", rect.can_hold(&small));

    // {:#?} prints in a readable multi-line form.
    println!("{:#?}", small);

    let red = Color(255, 0, 0);
    println!("red = ({}, {}, {})", red.0, red.1, red.2);
    let _marker = Marker;

    // Struct update syntax: copy the rest of the fields from another value.
    let first = Player { name: String::from("Ada"), score: 10, active: true };
    let second = Player { name: String::from("Linus"), ..first };
    println!("{second:?}");
    // `first.name` was not copied, so `first` is only partly usable now.

    // Try it: add a method `perimeter(&self) -> f64` to Rectangle and print it.
}
