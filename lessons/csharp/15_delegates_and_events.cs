// Lesson 15: Delegates, lambdas and events

// A delegate is a type that holds a method. Func<> and Action<> cover most needs:
//   Func<A, B, R>  takes A and B, returns R        Action<A>  takes A, returns nothing
Func<int, int, int> add = (a, b) => a + b;
Action<string> print = Console.WriteLine;
print($"add(2, 3) = {add(2, 3)}");

// Passing behavior into a method.
Console.WriteLine(string.Join(", ", Transform(new[] { 1, 2, 3 }, n => n * 10)));

// A delegate can hold several methods (a "multicast" delegate).
Action<string> logger = s => Console.WriteLine("log: " + s);
logger += s => Console.WriteLine("also: " + s);
logger("started");

// Closures: a lambda remembers the variables around it.
int factor = 3;
Func<int, int> times = n => n * factor;
factor = 4;                                  // the lambda sees the NEW value
Console.WriteLine(times(5));

// A custom delegate type.
Comparison<string> byLength = (x, y) => x.Length.CompareTo(y.Length);
var words = new List<string> { "banana", "fig", "apple" };
words.Sort(byLength);
Console.WriteLine(string.Join(", ", words));

// Events: a class tells others that something happened, without knowing who listens.
var thermostat = new Thermostat();
thermostat.TemperatureChanged += (sender, e) => Console.WriteLine($"  display: now {e.NewValue} degrees");
thermostat.TemperatureChanged += (sender, e) =>
{
    if (e.NewValue > 25) Console.WriteLine("  alarm: too hot!");
};
thermostat.Set(20);
thermostat.Set(20);        // no change: no event
thermostat.Set(28);

// Unsubscribing.
EventHandler<TemperatureEventArgs> once = (s, e) => Console.WriteLine("  (this runs once)");
thermostat.TemperatureChanged += once;
thermostat.Set(18);
thermostat.TemperatureChanged -= once;
thermostat.Set(19);

static IEnumerable<int> Transform(IEnumerable<int> items, Func<int, int> f)
{
    foreach (var item in items) yield return f(item);
}

class TemperatureEventArgs(int oldValue, int newValue) : EventArgs
{
    public int OldValue { get; } = oldValue;
    public int NewValue { get; } = newValue;
}

class Thermostat
{
    private int _temperature;

    public event EventHandler<TemperatureEventArgs>? TemperatureChanged;

    public void Set(int value)
    {
        if (value == _temperature) return;
        var old = _temperature;
        _temperature = value;
        TemperatureChanged?.Invoke(this, new TemperatureEventArgs(old, value));   // ?. : only if someone listens
    }
}

// Try it: add a Minimum property and raise a "TooCold" event when the value drops below it.
