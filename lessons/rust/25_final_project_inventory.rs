// Lesson 25: Final project: a small inventory manager
//
// This program uses nearly everything from the earlier lessons: structs, enums, traits,
// collections, iterators, closures, error handling and a little text parsing.
// It runs a built-in list of commands so that you can just press F5 and read the output.

use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
struct Item {
    name: String,
    quantity: u32,
    price_cents: u64,
}

#[derive(Debug, PartialEq)]
enum InventoryError {
    UnknownItem(String),
    NotEnoughStock { name: String, have: u32, wanted: u32 },
    BadCommand(String),
}

impl fmt::Display for InventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InventoryError::UnknownItem(name) => write!(f, "we do not stock '{name}'"),
            InventoryError::NotEnoughStock { name, have, wanted } => {
                write!(f, "only {have} of {name} left, you asked for {wanted}")
            }
            InventoryError::BadCommand(text) => write!(f, "cannot understand: {text}"),
        }
    }
}

#[derive(Default)]
struct Inventory {
    items: HashMap<String, Item>,
}

impl Inventory {
    fn add(&mut self, name: &str, quantity: u32, price_cents: u64) {
        let item = self
            .items
            .entry(name.to_string())
            .or_insert(Item { name: name.to_string(), quantity: 0, price_cents });
        item.quantity += quantity;
        item.price_cents = price_cents; // the newest price wins
    }

    fn remove(&mut self, name: &str, quantity: u32) -> Result<(), InventoryError> {
        let item = self.items.get_mut(name).ok_or_else(|| InventoryError::UnknownItem(name.to_string()))?;
        if item.quantity < quantity {
            return Err(InventoryError::NotEnoughStock { name: name.to_string(), have: item.quantity, wanted: quantity });
        }
        item.quantity -= quantity;
        Ok(())
    }

    fn total_value_cents(&self) -> u64 {
        self.items.values().map(|i| i.quantity as u64 * i.price_cents).sum()
    }

    fn low_stock(&self, limit: u32) -> Vec<&Item> {
        let mut low: Vec<&Item> = self.items.values().filter(|i| i.quantity <= limit).collect();
        low.sort_by(|a, b| a.name.cmp(&b.name));
        low
    }

    // One text command, for example "add apple 10 50" or "remove apple 3".
    fn run(&mut self, command: &str) -> Result<String, InventoryError> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        let bad = || InventoryError::BadCommand(command.to_string());
        match parts.as_slice() {
            ["add", name, quantity, price] => {
                let quantity = quantity.parse().map_err(|_| bad())?;
                let price = price.parse().map_err(|_| bad())?;
                self.add(name, quantity, price);
                Ok(format!("added {quantity} {name}"))
            }
            ["remove", name, quantity] => {
                let quantity = quantity.parse().map_err(|_| bad())?;
                self.remove(name, quantity)?;
                Ok(format!("removed {quantity} {name}"))
            }
            ["report"] => Ok(self.report()),
            _ => Err(bad()),
        }
    }

    fn report(&self) -> String {
        let mut names: Vec<&String> = self.items.keys().collect();
        names.sort();
        let mut lines: Vec<String> = names
            .iter()
            .map(|name| {
                let item = &self.items[*name];
                format!("  {:<8} x{:<3} @ {}", item.name, item.quantity, money(item.price_cents))
            })
            .collect();
        lines.push(format!("  total value: {}", money(self.total_value_cents())));
        lines.join("\n")
    }
}

fn money(cents: u64) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

fn main() {
    let mut inventory = Inventory::default();
    let script = [
        "add apple 10 50",
        "add pear 4 80",
        "add melon 1 450",
        "remove apple 3",
        "remove pear 9",
        "remove kiwi 1",
        "sell apple",
        "add apple 5 55",
        "report",
    ];
    for command in script {
        match inventory.run(command) {
            Ok(message) => println!("> {command}\n  {message}"),
            Err(error) => println!("> {command}\n  ERROR: {error}"),
        }
    }
    let low: Vec<String> = inventory.low_stock(4).iter().map(|i| i.name.clone()).collect();
    println!("low stock: {low:?}");

    // Ideas to extend it:
    //  - read the commands from the keyboard (lesson 21) in a loop until the user types "quit"
    //  - save the inventory to a file and load it at start (lesson 21)
    //  - add a #[cfg(test)] module that tests Inventory (lesson 23)
    //  - build it as a Cargo project with File > New project and split it into modules (lesson 19)
}
