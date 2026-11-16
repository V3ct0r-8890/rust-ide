// Lesson 20: Smart pointers: Box, Rc and RefCell

use std::cell::RefCell;
use std::rc::Rc;

// Box<T> puts a value on the heap. It is needed for types whose size is not known,
// such as a type that contains itself.
#[derive(Debug)]
enum List {
    Item(i32, Box<List>),
    End,
}

use List::{End, Item};

fn sum(list: &List) -> i32 {
    match list {
        Item(value, rest) => value + sum(rest),
        End => 0,
    }
}

// Rc<T>: reference counting, so SEVERAL owners can share one value (single thread).
// RefCell<T>: change a value through a shared reference; the borrow rules are checked
// while the program runs instead of while compiling.
#[derive(Debug)]
struct Node {
    name: String,
    children: RefCell<Vec<Rc<Node>>>,
}

fn main() {
    let list = Item(1, Box::new(Item(2, Box::new(Item(3, Box::new(End))))));
    println!("{list:?}");
    println!("sum = {}", sum(&list));

    // A Box is dropped (freed) when it goes out of scope, like any owner.
    let boxed = Box::new(41);
    println!("{}", *boxed + 1);

    // Rc: clone the pointer, not the data.
    let shared = Rc::new(String::from("shared text"));
    println!("owners: {}", Rc::strong_count(&shared));
    let second = Rc::clone(&shared);
    {
        let third = Rc::clone(&shared);
        println!("owners: {} ({third})", Rc::strong_count(&shared));
    }
    println!("owners: {} ({second})", Rc::strong_count(&shared));

    // RefCell: interior mutability.
    let counter = RefCell::new(0);
    *counter.borrow_mut() += 1;
    *counter.borrow_mut() += 1;
    println!("counter = {}", counter.borrow());
    // Two borrow_mut() alive at once would panic at run time.

    // Rc<RefCell<T>>: shared AND changeable.
    let wallet = Rc::new(RefCell::new(100));
    let a = Rc::clone(&wallet);
    let b = Rc::clone(&wallet);
    *a.borrow_mut() -= 30;
    *b.borrow_mut() += 5;
    println!("wallet = {}", wallet.borrow());

    // A tree where the children are shared.
    let leaf = Rc::new(Node { name: "leaf".into(), children: RefCell::new(vec![]) });
    let branch = Rc::new(Node { name: "branch".into(), children: RefCell::new(vec![Rc::clone(&leaf)]) });
    println!("{} has {} child; leaf is shared by {} owners", branch.name, branch.children.borrow().len(), Rc::strong_count(&leaf));

    // For threads use Arc and Mutex instead (lesson 24).
    // Try it: add a second branch that also holds `leaf` and print the owner count.
}
