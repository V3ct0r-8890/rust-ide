// Lesson 14: HashMap, HashSet and BTreeMap

use std::collections::{BTreeMap, HashMap, HashSet};

fn main() {
    // A HashMap stores key -> value pairs with fast lookup. The order is NOT defined.
    let mut ages: HashMap<String, u32> = HashMap::new();
    ages.insert("Ada".to_string(), 36);
    ages.insert("Linus".to_string(), 28);
    ages.insert("Grace".to_string(), 45);
    println!("{} people", ages.len());

    // get returns an Option, because the key might not exist.
    println!("{:?}", ages.get("Ada"));
    println!("{:?}", ages.get("Nobody"));
    if let Some(age) = ages.get("Grace") {
        println!("Grace is {age}");
    }
    println!("{}", ages.contains_key("Linus"));

    // Inserting an existing key replaces the value.
    ages.insert("Ada".to_string(), 37);
    // get_mut gives a mutable reference.
    if let Some(age) = ages.get_mut("Linus") {
        *age += 1;
    }
    ages.remove("Grace");

    // Iterate (sorted here only so the output is always the same).
    let mut names: Vec<_> = ages.keys().collect();
    names.sort();
    for name in names {
        println!("{name}: {}", ages[name]);
    }

    // The entry API: "insert if missing, then change". Perfect for counting.
    let text = "the cat and the hat and the bat";
    let mut counts: HashMap<&str, i32> = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word).or_insert(0) += 1;
    }
    let mut pairs: Vec<_> = counts.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0))); // most frequent first
    println!("{pairs:?}");

    // A HashSet keeps unique values.
    let mut seen = HashSet::new();
    for n in [3, 1, 3, 2, 1] {
        let new = seen.insert(n); // false if it was already there
        println!("{n} new? {new}");
    }
    println!("{} unique", seen.len());

    let a: HashSet<i32> = [1, 2, 3, 4].into_iter().collect();
    let b: HashSet<i32> = [3, 4, 5].into_iter().collect();
    let mut both: Vec<_> = a.intersection(&b).collect();
    both.sort();
    let mut all: Vec<_> = a.union(&b).collect();
    all.sort();
    println!("both {both:?}, all {all:?}");

    // A BTreeMap keeps its keys sorted.
    let mut sorted = BTreeMap::new();
    sorted.insert("pear", 3);
    sorted.insert("apple", 5);
    sorted.insert("fig", 1);
    println!("{sorted:?}");

    // Try it: count how often each letter appears in a sentence.
}
