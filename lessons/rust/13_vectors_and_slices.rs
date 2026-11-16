// Lesson 13: Vectors, arrays and slices

fn sum(numbers: &[i32]) -> i32 {
    // &[i32] is a slice: a view of any array or Vec, so the function works with both.
    let mut total = 0;
    for n in numbers {
        total += n;
    }
    total
}

fn main() {
    // An array has a fixed size known at compile time.
    let days = ["Mon", "Tue", "Wed"];
    println!("{} days, first is {}", days.len(), days[0]);
    let zeros = [0; 5]; // five zeros
    println!("{zeros:?}");

    // A Vec grows and shrinks.
    let mut numbers = vec![5, 3, 8];
    numbers.push(1);
    numbers.push(9);
    println!("{numbers:?} length {}", numbers.len());

    let last = numbers.pop(); // removes and returns the last item as an Option
    println!("popped {last:?}");
    numbers.insert(1, 100); // insert at a position
    numbers.remove(0); // remove at a position
    println!("{numbers:?}");

    // Indexing crashes the program if the position is outside; get() returns an Option instead.
    println!("{}", numbers[0]);
    println!("{:?} {:?}", numbers.get(1), numbers.get(99));

    // Looping.
    for n in &numbers {
        print!("{n} ");
    }
    println!();
    for n in numbers.iter_mut() {
        *n += 1; // change each item in place
    }
    println!("{numbers:?}");
    for (i, n) in numbers.iter().enumerate() {
        println!("#{i} = {n}");
    }

    // Slices: a part of a Vec or array.
    let part = &numbers[1..3];
    println!("{part:?} sum = {}", sum(part));
    println!("whole sum = {}", sum(&numbers));
    println!("array sum = {}", sum(&[1, 2, 3]));

    // Useful methods.
    let mut words = vec!["pear", "apple", "fig"];
    words.sort();
    println!("{words:?}");
    words.reverse();
    println!("{words:?} contains fig: {}", words.contains(&"fig"));
    let mut values = vec![4, 1, 4, 2, 1];
    values.sort();
    values.dedup(); // removes neighbors that are equal
    println!("{values:?}");
    values.retain(|v| *v != 2); // keep only what matches
    println!("{values:?}");
    values.extend([7, 8]);
    println!("{values:?} first {:?} last {:?}", values.first(), values.last());

    // Vec of Vec: a grid.
    let mut grid = vec![vec![0; 3]; 2];
    grid[1][2] = 5;
    println!("{grid:?}");

    // Try it: find the largest number in a Vec without using .iter().max().
}
