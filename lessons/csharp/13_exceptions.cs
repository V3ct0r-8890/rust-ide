// Lesson 13: Exceptions
//
// When something goes wrong, .NET "throws" an exception. If nothing catches it the program stops
// and prints it. try/catch lets you handle the problem.

try
{
    Console.WriteLine(Divide(10, 2));
    Console.WriteLine(Divide(1, 0));
    Console.WriteLine("never reached");
}
catch (DivideByZeroException)
{
    Console.WriteLine("cannot divide by zero");
}

// Several catch blocks (most specific first), the exception object, finally.
foreach (var text in new[] { "42", "forty", null, "99999999999" })
{
    try
    {
        int number = int.Parse(text!);
        Console.WriteLine($"{text}: ok, {number}");
    }
    catch (FormatException e)
    {
        Console.WriteLine($"{text}: not a number ({e.Message})");
    }
    catch (ArgumentNullException)
    {
        Console.WriteLine("no text given");
    }
    catch (OverflowException)
    {
        Console.WriteLine($"{text}: too big");
    }
    finally
    {
        Console.WriteLine("  (finally always runs)");
    }
}

// Throwing your own.
foreach (var age in new[] { 5, -1 })
{
    try
    {
        Console.WriteLine(SetAge(age));
    }
    catch (ArgumentOutOfRangeException e)
    {
        Console.WriteLine("rejected: " + e.Message);
    }
}

// Custom exception classes.
try
{
    Withdraw(10, 50);
}
catch (InsufficientFundsException e)
{
    Console.WriteLine($"error: {e.Message} (missing {e.Wanted - e.Balance})");
}

// Wrapping an exception keeps the original as InnerException.
try
{
    try
    {
        int.Parse("eighty");
    }
    catch (FormatException e)
    {
        throw new InvalidOperationException("bad port setting", e);
    }
}
catch (InvalidOperationException e)
{
    Console.WriteLine($"{e.Message} | caused by {e.InnerException!.GetType().Name}");
}

// Exception filters.
try
{
    throw new IOException("disk full");
}
catch (IOException e) when (e.Message.Contains("full"))
{
    Console.WriteLine("handled the full disk");
}

// Missing files are a common failure.
try
{
    File.ReadAllText("this-file-does-not-exist.txt");
}
catch (FileNotFoundException e)
{
    Console.WriteLine("no file: " + Path.GetFileName(e.FileName));
}

// Prefer TryParse and checks for expected situations; exceptions are for the unexpected.
Console.WriteLine(int.TryParse("12", out var n) ? n : -1);

// using: always clean up (it calls Dispose, even when an exception happens).
using (var guard = new Guard("work"))
{
    Console.WriteLine("  working");
}

// ---- helpers ----
static double Divide(int a, int b) => a / b;      // int / 0 throws; double / 0 would give Infinity

static int SetAge(int age)
{
    if (age < 0) throw new ArgumentOutOfRangeException(nameof(age), $"age cannot be negative: {age}");
    return age;
}

static decimal Withdraw(decimal balance, decimal amount)
{
    if (amount > balance) throw new InsufficientFundsException(balance, amount);
    return balance - amount;
}

class InsufficientFundsException(decimal balance, decimal wanted)
    : Exception($"balance {balance} is less than {wanted}")
{
    public decimal Balance { get; } = balance;
    public decimal Wanted { get; } = wanted;
}

class Guard : IDisposable
{
    private readonly string _label;
    public Guard(string label) { _label = label; Console.WriteLine($"start {label}"); }
    public void Dispose() => Console.WriteLine($"end {_label}");
}

// Try it: write a loop that keeps asking for a number with Console.ReadLine() until it gets a valid one.
