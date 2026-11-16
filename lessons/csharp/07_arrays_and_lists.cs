// Lesson 7: Arrays and lists

// An array has a fixed size.
int[] numbers = { 5, 3, 8, 1 };
int[] zeros = new int[5];                 // five zeros
Console.WriteLine($"{numbers.Length} numbers, first {numbers[0]}, last {numbers[^1]}");
numbers[0] = 50;
Console.WriteLine(string.Join(", ", numbers));

Array.Sort(numbers);
Console.WriteLine(string.Join(", ", numbers));
Array.Reverse(numbers);
Console.WriteLine(string.Join(", ", numbers));
Console.WriteLine(Array.IndexOf(numbers, 8));
Console.WriteLine(string.Join(", ", numbers[1..3]));         // a range
Console.WriteLine(zeros.Length);

// A List<T> grows and shrinks.
var list = new List<int> { 5, 3, 8 };
list.Add(1);
list.AddRange(new[] { 9, 2 });
list.Insert(1, 100);
Console.WriteLine($"{string.Join(", ", list)} (count {list.Count})");

list.Remove(100);            // by value
list.RemoveAt(0);            // by position
Console.WriteLine(string.Join(", ", list));
Console.WriteLine($"{list.Contains(8)} {list.IndexOf(8)}");

list.Sort();
Console.WriteLine(string.Join(", ", list));
list.Sort((a, b) => b.CompareTo(a));         // custom order: descending
Console.WriteLine(string.Join(", ", list));

// Looping.
foreach (int n in list) Console.Write(n + " ");
Console.WriteLine();
for (int i = 0; i < list.Count; i++)
    list[i] *= 2;                            // changing needs a for loop (foreach cannot assign)
Console.WriteLine(string.Join(", ", list));

// Useful methods.
Console.WriteLine($"{list.Sum()} {list.Min()} {list.Max()} {list.Average()}");
Console.WriteLine(list.Find(n => n > 10));
Console.WriteLine(string.Join(", ", list.FindAll(n => n > 10)));
list.RemoveAll(n => n < 5);
Console.WriteLine(string.Join(", ", list));

// Copying: assignment does NOT copy the list.
var a = new List<int> { 1, 2, 3 };
var b = a;
b.Add(4);
Console.WriteLine(a.Count);                  // 4
var c = new List<int>(a);                    // a real copy
c.Add(5);
Console.WriteLine($"{a.Count} {c.Count}");

// Two-dimensional arrays and lists of lists.
int[,] grid = new int[2, 3];
grid[1, 2] = 5;
Console.WriteLine($"{grid.GetLength(0)}x{grid.GetLength(1)} value {grid[1, 2]}");
var rows = new List<List<int>> { new() { 1, 2 }, new() { 3 } };
Console.WriteLine(rows[0][1]);

// Spans and collection expressions (C# 12).
List<string> names = ["Ada", "Linus", "Grace"];
string[] copy = [.. names, "Alan"];          // .. spreads another collection
Console.WriteLine(string.Join(", ", copy));

// Try it: remove the duplicates from { 3, 1, 3, 2, 1 } but keep the first appearances in order.
