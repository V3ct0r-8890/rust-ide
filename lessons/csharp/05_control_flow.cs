// Lesson 5: Control flow: if, loops and switch

int temperature = 22;
if (temperature > 30)
{
    Console.WriteLine("hot");
}
else if (temperature > 15)
{
    Console.WriteLine("pleasant");
}
else
{
    Console.WriteLine("cold");
}

// Comparisons: == != < > <= >=   Logic: && (and), || (or), ! (not)
bool weekend = false;
if (!weekend && temperature > 20)
    Console.WriteLine("work day, nice weather");   // braces are optional for one statement

// The conditional ("ternary") operator.
string label = temperature % 2 == 0 ? "even" : "odd";
Console.WriteLine(label);

// for: start; condition; step.
for (int i = 1; i <= 3; i++)
    Console.WriteLine($"i = {i}");

// foreach goes through any collection.
foreach (char letter in "abc")
    Console.WriteLine(letter);
foreach (var fruit in new[] { "apple", "pear" })
    Console.WriteLine(fruit);

// while and do-while (do-while always runs at least once).
int n = 3;
while (n > 0)
{
    Console.WriteLine($"countdown {n}");
    n--;
}
int tries = 0;
do
{
    tries++;
} while (tries < 3);
Console.WriteLine($"tries = {tries}");

// break leaves a loop, continue skips to the next round.
for (int i = 1; i <= 10; i++)
{
    if (i % 2 == 0) continue;
    if (i > 7) break;
    Console.WriteLine($"odd {i}");
}

// switch statement.
int day = 3;
switch (day)
{
    case 1:
    case 7:
        Console.WriteLine("weekend");
        break;
    case >= 2 and <= 6:                 // patterns
        Console.WriteLine("weekday");
        break;
    default:
        Console.WriteLine("unknown");
        break;
}

// switch expression: gives a value, must cover every case.
int number = 3;
string size = number switch
{
    < 0 => "negative",
    0 => "zero",
    1 or 2 or 3 => "small",
    < 100 => "medium",
    _ => "large",
};
Console.WriteLine(size);

// Pattern matching with types and properties.
object thing = 42;
if (thing is int whole && whole > 40)
    Console.WriteLine($"a big int: {whole}");
string describe = thing switch
{
    int i when i % 2 == 0 => "even int",
    int => "odd int",
    string s => $"string of {s.Length}",
    null => "nothing",
    _ => "something else",
};
Console.WriteLine(describe);

// Try it: print the numbers 1 to 20, but "Fizz" for multiples of 3, "Buzz" for 5, "FizzBuzz" for both.
