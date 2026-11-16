// Lesson 16: Enums, records and pattern matching

// Enums: a fixed set of named values.
Weekday today = Weekday.Wednesday;
Console.WriteLine($"{today} = {(int)today}");
Console.WriteLine(today == Weekday.Wednesday);
Console.WriteLine(Enum.Parse<Weekday>("Friday"));
Console.WriteLine(Enum.TryParse<Weekday>("Funday", out _));
Console.WriteLine(string.Join(", ", Enum.GetNames<Weekday>()));
Console.WriteLine(IsWeekend(Weekday.Sunday) + " " + IsWeekend(today));

// [Flags] enums combine several options with | and test with HasFlag.
var access = Permissions.Read | Permissions.Write;
Console.WriteLine(access);
Console.WriteLine(access.HasFlag(Permissions.Write) + " " + access.HasFlag(Permissions.Delete));

// Records are immutable data with value equality and `with`.
var ada = new Person("Ada", "Lovelace", 36);
var older = ada with { Age = 37 };
Console.WriteLine($"{ada}\n{older}\n{ada == older} {ada == new Person("Ada", "Lovelace", 36)}");
var (first, last, _) = ada;                      // deconstruct
Console.WriteLine($"{first} {last}");

// Pattern matching with records and properties.
Shape[] shapes = { new Circle(1), new Rectangle(2, 3), new Triangle(3, 4, 5), new Circle(0) };
foreach (var shape in shapes)
{
    string text = shape switch
    {
        Circle { Radius: 0 } => "a point",
        Circle c => $"circle area {Math.PI * c.Radius * c.Radius:F2}",
        Rectangle(var w, var h) when w == h => $"square {w}",
        Rectangle r => $"rectangle {r.Width}x{r.Height}",
        Triangle t => $"triangle perimeter {t.A + t.B + t.C}",
        _ => "unknown",
    };
    Console.WriteLine(text);
}

// Property and list patterns.
object[] things = { 5, "hello", new[] { 1, 2, 3 }, null!, 3.5 };
foreach (var thing in things)
{
    var d = thing switch
    {
        int n and > 3 => "an int over 3",
        string { Length: > 3 } s => $"a long string: {s}",
        int[] and [1, .., 3] => "an array from 1 to 3",
        null => "null",
        _ => "something else",
    };
    Console.WriteLine(d);
}

// A tuple pattern.
Console.WriteLine(Describe(true, 0) + ", " + Describe(false, 5));

static string Describe(bool hot, int wind) => (hot, wind) switch
{
    (true, 0) => "hot and still",
    (true, _) => "hot and windy",
    (false, > 3) => "cold and windy",
    _ => "cold",
};

static bool IsWeekend(Weekday d) => d is Weekday.Saturday or Weekday.Sunday;

enum Weekday { Monday = 1, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday }

[Flags]
enum Permissions { None = 0, Read = 1, Write = 2, Delete = 4 }

record Person(string First, string Last, int Age);

abstract record Shape;
record Circle(double Radius) : Shape;
record Rectangle(double Width, double Height) : Shape;
record Triangle(double A, double B, double C) : Shape;

// Try it: add a Square record and handle it in the switch.
