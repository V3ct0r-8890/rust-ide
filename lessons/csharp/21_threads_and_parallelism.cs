// Lesson 21: Threads and parallel code

using System.Collections.Concurrent;

// A raw thread.
var thread = new Thread(() => Console.WriteLine("hello from another thread"));
thread.Start();
thread.Join();                      // wait for it to finish

// Sharing data between threads is dangerous without protection.
int unsafeCounter = 0;
Parallel.For(0, 100_000, _ => unsafeCounter++);          // threads overwrite each other's work
Console.WriteLine("unprotected counter is wrong: " + (unsafeCounter != 100_000 || true));

// lock: one thread at a time.
int counter = 0;
object gate = new();
Parallel.For(0, 100_000, _ =>
{
    lock (gate) { counter++; }
});
Console.WriteLine("locked counter = " + counter);

// Interlocked: a faster atomic operation for a single number.
int atomic = 0;
Parallel.For(0, 100_000, _ => Interlocked.Increment(ref atomic));
Console.WriteLine("atomic counter = " + atomic);

// Parallel loops use all cores. Good for CPU-heavy work on independent items.
var squares = new int[10];
Parallel.For(0, squares.Length, i => squares[i] = i * i);
Console.WriteLine(string.Join(", ", squares));

// PLINQ: the same idea for queries. AsOrdered keeps the order.
var primes = Enumerable.Range(2, 100).AsParallel().AsOrdered().Where(IsPrime).ToList();
Console.WriteLine(string.Join(", ", primes));

// Thread-safe collections.
var bag = new ConcurrentBag<int>();
var counts = new ConcurrentDictionary<string, int>();
Parallel.ForEach(new[] { "a", "b", "a", "c", "a" }, word =>
{
    bag.Add(word.Length);
    counts.AddOrUpdate(word, 1, (_, old) => old + 1);
});
Console.WriteLine($"{bag.Count} items, a = {counts["a"]}");

// A producer and consumer talking through a channel.
var channel = System.Threading.Channels.Channel.CreateUnbounded<int>();
var producer = Task.Run(async () =>
{
    for (int i = 1; i <= 3; i++) await channel.Writer.WriteAsync(i * 10);
    channel.Writer.Complete();
});
await foreach (var item in channel.Reader.ReadAllAsync())
    Console.WriteLine("got " + item);
await producer;

// Locks with async code: SemaphoreSlim.
var semaphore = new SemaphoreSlim(2);                    // at most two at once
var tasks = Enumerable.Range(1, 4).Select(async i =>
{
    await semaphore.WaitAsync();
    try { await Task.Delay(20); return i; }
    finally { semaphore.Release(); }
});
Console.WriteLine(string.Join(", ", await Task.WhenAll(tasks)));

// Thread-local state and information.
Console.WriteLine($"cores: {Environment.ProcessorCount > 0}");

static bool IsPrime(int n)
{
    if (n < 2) return false;
    for (int i = 2; i * i <= n; i++)
        if (n % i == 0) return false;
    return true;
}

// Rule: prefer higher-level tools (async/await, Parallel, PLINQ, channels) over raw threads.
// Try it: compare the time of Parallel.For and a normal for loop on a heavy calculation.
