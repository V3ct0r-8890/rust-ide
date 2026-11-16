// Lesson 8: Dictionaries and sets

// A Dictionary maps keys to values.
var ages = new Dictionary<string, int>
{
    ["Ada"] = 36,
    ["Linus"] = 28,
};
ages["Grace"] = 45;                  // add
ages["Ada"] = 37;                    // replace
Console.WriteLine(ages["Ada"]);
Console.WriteLine(ages.ContainsKey("Linus"));

// A missing key throws, so use TryGetValue.
if (ages.TryGetValue("Nobody", out int age))
    Console.WriteLine(age);
else
    Console.WriteLine("no such person");
Console.WriteLine(ages.GetValueOrDefault("Nobody", -1));

ages.Remove("Linus");
Console.WriteLine(ages.Count);

foreach (var (name, years) in ages)       // deconstruct each pair
    Console.WriteLine($"{name} is {years}");
Console.WriteLine(string.Join(", ", ages.Keys));
Console.WriteLine(string.Join(", ", ages.Values));

try
{
    Console.WriteLine(ages["Nobody"]);
}
catch (KeyNotFoundException e)
{
    Console.WriteLine("error: " + e.Message);
}

// Counting words.
string text = "the cat and the hat and the bat";
var counts = new Dictionary<string, int>();
foreach (var word in text.Split(' '))
    counts[word] = counts.GetValueOrDefault(word) + 1;
foreach (var pair in counts.OrderByDescending(p => p.Value).ThenBy(p => p.Key))
    Console.WriteLine($"{pair.Key}: {pair.Value}");

// The same with LINQ.
var grouped = text.Split(' ').GroupBy(w => w).ToDictionary(g => g.Key, g => g.Count());
Console.WriteLine(grouped["the"]);

// SortedDictionary keeps keys in order.
var sorted = new SortedDictionary<string, int> { ["pear"] = 3, ["apple"] = 5, ["fig"] = 1 };
Console.WriteLine(string.Join(", ", sorted.Select(p => $"{p.Key}={p.Value}")));

// A HashSet keeps unique values.
var seen = new HashSet<int>();
foreach (int n in new[] { 3, 1, 3, 2, 1 })
    Console.WriteLine($"{n} new? {seen.Add(n)}");        // Add returns false if already there
Console.WriteLine(seen.Count);

var a = new HashSet<int> { 1, 2, 3, 4 };
var b = new HashSet<int> { 3, 4, 5 };
var both = new HashSet<int>(a); both.IntersectWith(b);
var all = new HashSet<int>(a); all.UnionWith(b);
var onlyA = new HashSet<int>(a); onlyA.ExceptWith(b);
Console.WriteLine($"both {string.Join(",", both)} | all {string.Join(",", all)} | only a {string.Join(",", onlyA)}");

// Queue (first in, first out) and Stack (last in, first out).
var queue = new Queue<string>();
queue.Enqueue("first"); queue.Enqueue("second");
Console.WriteLine(queue.Dequeue());
var stack = new Stack<string>();
stack.Push("bottom"); stack.Push("top");
Console.WriteLine(stack.Pop());

// Try it: count how often each letter appears in a sentence.
