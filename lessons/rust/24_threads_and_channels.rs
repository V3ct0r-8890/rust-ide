// Lesson 24: Threads, channels and shared state

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() {
    // spawn starts a thread. join waits for it and gives back its result.
    let handle = thread::spawn(|| {
        let mut total = 0u64;
        for i in 1..=1000 {
            total += i;
        }
        total
    });
    println!("sum computed in a thread: {}", handle.join().unwrap());

    // `move` hands data over to the thread.
    let words = vec!["alpha", "beta", "gamma"];
    let handle = thread::spawn(move || words.iter().map(|w| w.len()).sum::<usize>());
    println!("letters: {}", handle.join().unwrap());

    // Channels: threads send messages to each other (mpsc = many producers, one consumer).
    let (sender, receiver) = mpsc::channel();
    let mut workers = Vec::new();
    for id in 1..=3 {
        let sender = sender.clone();
        workers.push(thread::spawn(move || {
            thread::sleep(Duration::from_millis(10 * id));
            sender.send(format!("worker {id} finished")).unwrap();
        }));
    }
    drop(sender); // the receiver stops when all senders are gone
    let mut messages: Vec<String> = receiver.iter().collect();
    messages.sort();
    for message in messages {
        println!("{message}");
    }
    for worker in workers {
        worker.join().unwrap();
    }

    // Shared state: Arc (shared ownership across threads) + Mutex (one at a time may change it).
    let counter = Arc::new(Mutex::new(0));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                *counter.lock().unwrap() += 1;
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    println!("counter = {}", *counter.lock().unwrap()); // always 8000

    // Atomics are a lighter way to share a single number.
    let hits = Arc::new(AtomicUsize::new(0));
    thread::scope(|scope| {
        // Scoped threads may borrow local variables, because they all end before the scope ends.
        for _ in 0..4 {
            scope.spawn(|| {
                hits.fetch_add(5, Ordering::SeqCst);
            });
        }
    });
    println!("hits = {}", hits.load(Ordering::SeqCst));

    // Rust's ownership rules turn most data races into compile errors.
    // Try it: replace Arc<Mutex<i32>> with Rc<RefCell<i32>> and read the compiler's complaint.
}
