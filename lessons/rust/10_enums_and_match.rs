// Lesson 10: Enums and pattern matching
//
// An enum is a value that is ONE of several variants. Each variant can carry data.

#[derive(Debug)]
enum Direction {
    North,
    South,
    East,
    West,
}

#[derive(Debug)]
enum Shape {
    Circle(f64),                      // radius
    Rectangle { width: f64, height: f64 }, // named fields
    Triangle(f64, f64, f64),          // three sides
    Point,                            // no data
}

impl Shape {
    fn area(&self) -> f64 {
        // match must handle every variant: add a variant and the compiler tells you where to update.
        match self {
            Shape::Circle(r) => std::f64::consts::PI * r * r,
            Shape::Rectangle { width, height } => width * height,
            Shape::Triangle(a, b, c) => {
                let s = (a + b + c) / 2.0; // Heron's formula
                (s * (s - a) * (s - b) * (s - c)).sqrt()
            }
            Shape::Point => 0.0,
        }
    }
}

fn turn_right(d: Direction) -> Direction {
    match d {
        Direction::North => Direction::East,
        Direction::East => Direction::South,
        Direction::South => Direction::West,
        Direction::West => Direction::North,
    }
}

#[derive(Debug)]
enum Message {
    Quit,
    Write(String),
    Move { x: i32, y: i32 },
}

fn main() {
    let d = turn_right(Direction::North);
    println!("{d:?}");

    let shapes = [
        Shape::Circle(1.5),
        Shape::Rectangle { width: 2.0, height: 3.0 },
        Shape::Triangle(3.0, 4.0, 5.0),
        Shape::Point,
    ];
    for shape in &shapes {
        println!("{shape:?} -> area {:.2}", shape.area());
    }

    // Patterns can test conditions with a guard (if), and bind values with @.
    let numbers = [-5, 0, 7, 120];
    for n in numbers {
        let kind = match n {
            i32::MIN..=-1 => "negative",
            0 => "zero",
            x if x % 2 == 1 => "odd positive",
            big @ 100.. => {
                println!("(big value {big})");
                "big even"
            }
            _ => "even positive",
        };
        println!("{n}: {kind}");
    }

    let messages = vec![Message::Write(String::from("hi")), Message::Move { x: 1, y: -2 }, Message::Quit];
    for m in messages {
        match m {
            Message::Quit => println!("quit"),
            Message::Write(text) => println!("write {text}"),
            Message::Move { x, y } => println!("move to {x},{y}"),
        }
    }

    // if let: match just one pattern when you do not care about the rest.
    let maybe = Message::Write(String::from("only this one"));
    if let Message::Write(text) = maybe {
        println!("got text: {text}");
    }

    // Try it: add a variant `Pause` to Message and see what the compiler tells you.
}
