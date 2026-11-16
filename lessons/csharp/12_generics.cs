// Lesson 12: Generics
//
// Generics let one piece of code work for many types. <T> is a placeholder for "some type".
// You have used them already: List<int>, Dictionary<string, int>.

Console.WriteLine(Largest(new[] { 3, 9, 2 }));
Console.WriteLine(Largest(new[] { 1.5, 0.2 }));
Console.WriteLine(Largest(new[] { "pear", "apple" }));

var stack = new Stack<string>();
stack.Push("one");
stack.Push("two");
Console.WriteLine($"{stack.Peek()} {stack.Count}");
Console.WriteLine(stack.Pop());
Console.WriteLine(stack.Pop());
Console.WriteLine(stack.IsEmpty);

var pair = new Pair<string, int>("age", 36);
Console.WriteLine(pair);
Console.WriteLine(pair.Swap());

// Constraints say what T must be able to do.
Console.WriteLine(Sum(new[] { 1, 2, 3 }));
Console.WriteLine(Sum(new[] { 0.5, 0.25 }));
var animal = Create<Cat>();
Console.WriteLine(animal.Name);

// Generic methods with several types; the compiler infers them.
Console.WriteLine(string.Join(", ", Map(new[] { 1, 2, 3 }, n => $"#{n}")));

// default(T) is the "zero" of a type.
Console.WriteLine(Default<int>() + " " + (Default<string>() ?? "null"));

// ---- generic code ----
static T Largest<T>(IEnumerable<T> items) where T : IComparable<T>
{
    T best = items.First();
    foreach (var item in items)
        if (item.CompareTo(best) > 0)
            best = item;
    return best;
}

// INumber<T>: works for every number type (generic math).
static T Sum<T>(IEnumerable<T> items) where T : System.Numerics.INumber<T>
{
    T total = T.Zero;
    foreach (var item in items) total += item;
    return total;
}

static T Create<T>() where T : new() => new T();      // T needs a parameterless constructor

static IEnumerable<TResult> Map<TSource, TResult>(IEnumerable<TSource> source, Func<TSource, TResult> f)
{
    foreach (var item in source) yield return f(item);
}

static T? Default<T>() => default;

class Stack<T>
{
    private readonly List<T> _items = new();
    public void Push(T item) => _items.Add(item);
    public T Pop()
    {
        var item = _items[^1];
        _items.RemoveAt(_items.Count - 1);
        return item;
    }
    public T Peek() => _items[^1];
    public int Count => _items.Count;
    public bool IsEmpty => _items.Count == 0;
}

record Pair<TFirst, TSecond>(TFirst First, TSecond Second)
{
    public Pair<TSecond, TFirst> Swap() => new(Second, First);
}

class Cat
{
    public string Name { get; } = "a new cat";
}

// Try it: write a generic Queue<T> class with Enqueue, Dequeue and Count.
