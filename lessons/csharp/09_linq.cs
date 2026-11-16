// Lesson 9: LINQ: questions about collections
//
// LINQ (Language Integrated Query) gives you methods to filter, change, sort and combine any
// collection. Most of them are lazy: the work happens when you loop or call ToList().

var numbers = Enumerable.Range(1, 10).ToList();

Console.WriteLine(string.Join(", ", numbers.Where(n => n % 2 == 0)));
Console.WriteLine(string.Join(", ", numbers.Select(n => n * n)));
Console.WriteLine(numbers.Where(n => n % 2 == 1).Select(n => n * n).Sum());

Console.WriteLine($"{numbers.Min()} {numbers.Max()} {numbers.Average()} {numbers.Count()}");
Console.WriteLine($"{numbers.Any(n => n > 9)} {numbers.All(n => n > 0)}");
Console.WriteLine($"{numbers.First()} {numbers.Last()} {numbers.FirstOrDefault(n => n > 100)}");
Console.WriteLine(string.Join(", ", numbers.Take(3)));
Console.WriteLine(string.Join(", ", numbers.Skip(8)));
Console.WriteLine(string.Join(", ", numbers.Reverse<int>().Take(3)));
Console.WriteLine(numbers.Aggregate(1, (product, n) => product * n));    // fold

// Sorting.
var people = new List<Person>
{
    new("Ada", 36, "London"),
    new("Linus", 28, "Helsinki"),
    new("Grace", 45, "New York"),
    new("Alan", 41, "London"),
};
foreach (var p in people.OrderBy(p => p.Age))
    Console.WriteLine($"{p.Name} {p.Age}");
Console.WriteLine(string.Join(", ", people.OrderBy(p => p.City).ThenByDescending(p => p.Age).Select(p => p.Name)));

// Grouping.
foreach (var group in people.GroupBy(p => p.City).OrderBy(g => g.Key))
    Console.WriteLine($"{group.Key}: {string.Join(", ", group.Select(p => p.Name))}");

// Looking up and projecting to something new.
var oldest = people.MaxBy(p => p.Age);
Console.WriteLine(oldest);
var summary = people.Select(p => new { p.Name, IsAdult = p.Age >= 18 });   // an anonymous type
Console.WriteLine(summary.First());
Console.WriteLine(string.Join(", ", people.Select(p => p.Name).Distinct().Order()));

// Joining two collections.
var cities = new[] { new City("London", "UK"), new City("Helsinki", "Finland") };
var joined = people.Join(cities, p => p.City, c => c.Name, (p, c) => $"{p.Name} lives in {c.Country}");
Console.WriteLine(string.Join("; ", joined));

// Zip, chunk, set operations.
Console.WriteLine(string.Join(", ", new[] { "a", "b" }.Zip(new[] { 1, 2 }, (s, n) => s + n)));
Console.WriteLine(string.Join(" | ", numbers.Chunk(4).Select(c => string.Join(",", c))));
Console.WriteLine(string.Join(", ", new[] { 1, 2, 3 }.Intersect(new[] { 2, 3, 4 })));

// Query syntax is an alternative that looks like SQL.
var query = from p in people
            where p.Age > 30
            orderby p.Name
            select p.Name;
Console.WriteLine(string.Join(", ", query));

// Lazy evaluation: the query runs when you use it, not when you write it.
var lazy = numbers.Where(n => { Console.Write($"[check {n}]"); return n > 8; });
Console.WriteLine();
Console.WriteLine("created, nothing printed yet");
Console.WriteLine(string.Join(",", lazy));

// Try it: find the three longest words in "the quick brown fox jumps over the lazy dog".

record Person(string Name, int Age, string City);
record City(string Name, string Country);
