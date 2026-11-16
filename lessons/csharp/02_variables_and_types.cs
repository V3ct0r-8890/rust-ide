// Lesson 2: Variables and basic types
//
// C# is statically typed: every variable has a type that is fixed at compile time.

int age = 30;                 // whole number
double price = 19.99;         // floating point number
decimal money = 19.99m;       // exact decimal numbers: use for money (note the m)
bool ready = true;
char letter = 'R';            // one character, single quotes
string name = "Ada";          // text, double quotes
long big = 9_000_000_000;     // underscores make long numbers readable
byte small = 255;             // 0..255

Console.WriteLine($"{age} {price} {money} {ready} {letter} {name} {big} {small}");

// `var` lets the compiler work out the type (it is still fixed afterwards).
var count = 10;               // int
var title = "Report";         // string
Console.WriteLine($"{count.GetType()} {title.GetType()}");

// Change a variable.
age = age + 1;
age += 1;
age++;
Console.WriteLine(age);

// Constants never change.
const double Pi = 3.14159;
Console.WriteLine(Pi);

// Converting types.
double d = 3.99;
int truncated = (int)d;                 // explicit cast: cuts off the fraction
double widened = age;                   // implicit: safe, so no cast needed
Console.WriteLine($"{truncated} {widened}");
Console.WriteLine(int.Parse("42") + 1);
Console.WriteLine(Convert.ToString(255, 2));       // 255 in binary

// Parsing text safely.
if (int.TryParse("forty", out int parsed))
    Console.WriteLine(parsed);
else
    Console.WriteLine("not a number");

// Nullable types: a value type that may also be "no value".
int? maybe = null;
Console.WriteLine(maybe.HasValue);
maybe = 7;
Console.WriteLine(maybe.Value);
Console.WriteLine(maybe ?? -1);         // ?? gives a default when null

// Tuples group a few values.
(string, int) person = ("Ada", 36);
var point = (X: 3, Y: 4);
Console.WriteLine($"{person.Item1} {point.X},{point.Y}");

// Limits of the types.
Console.WriteLine($"{int.MaxValue} {int.MinValue} {double.MaxValue}");

// Try it: set `small` to 256 and see the compiler stop you.
