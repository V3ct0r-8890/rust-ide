// Lesson 20: async and await
//
// async/await lets a program wait (for a file, the network, a timer) WITHOUT blocking a thread.
// A method marked `async` returns a Task (or Task<T>); `await` pauses it until the Task finishes.

using System.Diagnostics;

var watch = Stopwatch.StartNew();

// One at a time: about 0.6 s.
string a = await FetchAsync("a", 300);
string b = await FetchAsync("b", 300);
Console.WriteLine($"{a}, {b}");

// At the same time: about 0.3 s. Start both tasks first, then await them.
watch.Restart();
var t1 = FetchAsync("c", 300);
var t2 = FetchAsync("d", 300);
var t3 = FetchAsync("e", 300);
string[] results = await Task.WhenAll(t1, t2, t3);
Console.WriteLine(string.Join(", ", results));
Console.WriteLine("ran together: " + (watch.ElapsedMilliseconds < 800));

// The first one to finish.
var fastest = await Task.WhenAny(FetchAsync("slow", 400), FetchAsync("fast", 50));
Console.WriteLine("first: " + await fastest);

// Errors flow through await.
try
{
    await FailAsync();
}
catch (InvalidOperationException e)
{
    Console.WriteLine("caught: " + e.Message);
}

// Cancellation and timeouts.
using (var cts = new CancellationTokenSource(TimeSpan.FromMilliseconds(100)))
{
    try
    {
        await FetchAsync("never", 5000, cts.Token);
    }
    catch (OperationCanceledException)
    {
        Console.WriteLine("cancelled after the timeout");
    }
}

// Run CPU work on another thread.
long sum = await Task.Run(() =>
{
    long total = 0;
    for (int i = 1; i <= 1_000_000; i++) total += i;
    return total;
});
Console.WriteLine(sum);

// Async streams: produce items over time with IAsyncEnumerable.
await foreach (int n in CountAsync(3))
    Console.WriteLine("tick " + n);

// Files have async versions too.
string path = Path.GetTempFileName();
await File.WriteAllTextAsync(path, "hello async");
Console.WriteLine(await File.ReadAllTextAsync(path));
File.Delete(path);

// Rules of thumb:
//  - Name async methods ...Async and return Task, not void (except event handlers).
//  - Do not call .Result or .Wait() on a Task: it can freeze a program (deadlock).
//  - async is about WAITING; Task.Run / Parallel is about computing on several cores.

static async Task<string> FetchAsync(string name, int milliseconds, CancellationToken token = default)
{
    await Task.Delay(milliseconds, token);       // pretend to call a server
    return $"{name} after {milliseconds}ms";
}

static async Task FailAsync()
{
    await Task.Delay(10);
    throw new InvalidOperationException("something broke");
}

static async IAsyncEnumerable<int> CountAsync(int count)
{
    for (int i = 1; i <= count; i++)
    {
        await Task.Delay(20);
        yield return i;
    }
}

// Try it: fetch five things at once and print each result as soon as it finishes (Task.WhenEach).
