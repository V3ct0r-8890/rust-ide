// Lesson 19: Modules and visibility
//
// Modules group code and control what is visible outside. In a real project each module is
// usually its own file (src/shapes.rs); here they are written inline with mod name { }.

mod geometry {
    // Everything is private by default. `pub` makes it visible to the outside.
    pub const UNIT: f64 = 1.0;

    pub struct Circle {
        pub radius: f64, // a public field
        id: u32,         // private: only code inside this module can touch it
    }

    impl Circle {
        pub fn new(radius: f64) -> Circle {
            Circle { radius, id: next_id() }
        }
        pub fn area(&self) -> f64 {
            std::f64::consts::PI * self.radius * self.radius
        }
        pub fn id(&self) -> u32 {
            self.id // a getter for the private field
        }
    }

    fn next_id() -> u32 {
        // private helper function
        42
    }

    // Modules nest.
    pub mod units {
        pub fn to_cm(meters: f64) -> f64 {
            meters * 100.0 * super::UNIT // `super` is the parent module
        }
    }
}

mod bank {
    pub struct Account {
        balance: i64, // private: nobody can set it to any value directly
    }

    impl Account {
        pub fn open() -> Account {
            Account { balance: 0 }
        }
        pub fn deposit(&mut self, amount: i64) -> Result<(), String> {
            if amount <= 0 {
                return Err("deposit must be positive".to_string());
            }
            self.balance += amount;
            Ok(())
        }
        pub fn balance(&self) -> i64 {
            self.balance
        }
    }
}

// `use` brings names into scope so you do not repeat the whole path.
use bank::Account;
use geometry::units::to_cm;

fn main() {
    let c = geometry::Circle::new(2.0);
    println!("area {:.2}, id {}, radius {}", c.area(), c.id(), c.radius);
    // println!("{}", c.id);   // <- error: field `id` is private

    println!("{}", to_cm(1.5));
    println!("{}", geometry::UNIT);

    let mut account = Account::open();
    account.deposit(50).unwrap();
    println!("{:?}", account.deposit(-5));
    println!("balance {}", account.balance());
    // account.balance = 1_000_000;   // <- error: private. The module protects its own rules.

    // In a real project:
    //   src/main.rs      mod geometry;   // loads src/geometry.rs
    //   src/geometry.rs  pub fn ...
    // and external crates are added to Cargo.toml and used with `use crate_name::...`.

    // Try it: add a `withdraw` method that refuses to go below zero.
}
