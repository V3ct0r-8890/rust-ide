// Lesson 19: Interfaces in practice: dependency injection and testable design
//
// Code that creates everything it needs is hard to test and to change. Instead, give a class
// what it needs through its constructor ("dependency injection"), as an interface.

var output = new List<string>();

// The real parts: a clock that reads the real time would make output change every run, so we
// pass a fixed clock here. In a real program you would pass a SystemClock.
var service = new GreetingService(new FixedClock(new DateTime(2026, 3, 14, 9, 0, 0)), new ConsoleLogger());
Console.WriteLine(service.Greet("Ada"));

var evening = new GreetingService(new FixedClock(new DateTime(2026, 3, 14, 20, 0, 0)), new ConsoleLogger());
Console.WriteLine(evening.Greet("Linus"));

// A fake logger collects messages so a test can check them.
var fake = new ListLogger();
var quiet = new GreetingService(new FixedClock(new DateTime(2026, 3, 14, 14, 0, 0)), fake);
quiet.Greet("Grace");
Console.WriteLine($"logged: {string.Join(" / ", fake.Messages)}");

// A tiny "container": a table from interface to implementation. Real programs use
// Microsoft.Extensions.DependencyInjection (a NuGet package) that does this for you.
var services = new Dictionary<Type, Func<object>>
{
    [typeof(ILogger)] = () => new ConsoleLogger(),
    [typeof(IClock)] = () => new FixedClock(new DateTime(2026, 3, 14, 7, 0, 0)),
};
T Resolve<T>() => (T)services[typeof(T)]();
var built = new GreetingService(Resolve<IClock>(), Resolve<ILogger>());
Console.WriteLine(built.Greet("Alan"));

// Interfaces also allow swapping behavior: the strategy pattern.
IDiscount[] rules = { new NoDiscount(), new PercentDiscount(10), new FlatDiscount(5) };
foreach (var rule in rules)
    Console.WriteLine($"{rule.GetType().Name}: {rule.Apply(50m)}");

interface IClock { DateTime Now { get; } }
interface ILogger { void Log(string message); }

class SystemClock : IClock { public DateTime Now => DateTime.Now; }
class FixedClock(DateTime now) : IClock { public DateTime Now { get; } = now; }

class ConsoleLogger : ILogger
{
    public void Log(string message) => Console.WriteLine("  [log] " + message);
}

class ListLogger : ILogger
{
    public List<string> Messages { get; } = new();
    public void Log(string message) => Messages.Add(message);
}

class GreetingService(IClock clock, ILogger logger)
{
    public string Greet(string name)
    {
        string part = clock.Now.Hour switch
        {
            < 12 => "Good morning",
            < 18 => "Good afternoon",
            _ => "Good evening",
        };
        logger.Log($"greeting {name}");
        return $"{part}, {name}!";
    }
}

interface IDiscount { decimal Apply(decimal price); }
class NoDiscount : IDiscount { public decimal Apply(decimal price) => price; }
class PercentDiscount(int percent) : IDiscount { public decimal Apply(decimal price) => price * (100 - percent) / 100; }
class FlatDiscount(decimal amount) : IDiscount { public decimal Apply(decimal price) => Math.Max(0, price - amount); }

// Try it: add an IDiscount that gives 20% off only above 100, and add it to the list.
