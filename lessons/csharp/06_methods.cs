// Lesson 6: Methods
//
// Methods (functions) can be written among the top-level statements. They are "local functions".
// Types (classes and others) must come AFTER the statements, at the bottom of the file.

Console.WriteLine(Add(2, 3));
Greet("Ada");
Greet("Linus", "Hi");
Greet(greeting: "Welcome", name: "Grace");      // named arguments: any order

// Return several values with a tuple.
var (low, high) = MinMax(new[] { 4, 9, -2, 7 });
Console.WriteLine($"{low} {high}");

// params: any number of arguments.
Console.WriteLine(Total(1, 2, 3, 4));

// out parameters: an extra way to return a value.
if (TryDivide(10, 4, out double result))
    Console.WriteLine(result);
Console.WriteLine(TryDivide(1, 0, out _));       // _ means "I do not need it"

// ref changes the caller's variable.
int x = 5;
Double(ref x);
Console.WriteLine(x);

// Recursion.
Console.WriteLine(Factorial(5));

// Methods are values: delegates and lambdas.
Func<int, int> square = n => n * n;
Console.WriteLine(square(7));
Console.WriteLine(ApplyTwice(square, 3));
Console.WriteLine(ApplyTwice(n => n + 10, 1));
Action<string> shout = s => Console.WriteLine(s.ToUpper() + "!");
shout("hello");

// Optional values, expression bodies.
Console.WriteLine(Area(2.5, 4));

// A local function can use the variables around it.
int counter = 0;
void Increase() => counter++;
Increase();
Increase();
Console.WriteLine(counter);

// ---- the methods ----
static int Add(int a, int b)
{
    return a + b;
}

static void Greet(string name, string greeting = "Hello")
{
    Console.WriteLine($"{greeting}, {name}!");
}

static (int Min, int Max) MinMax(int[] numbers)
{
    return (numbers.Min(), numbers.Max());
}

static int Total(params int[] numbers) => numbers.Sum();

static bool TryDivide(double a, double b, out double result)
{
    result = 0;
    if (b == 0) return false;
    result = a / b;
    return true;
}

static void Double(ref int value) => value *= 2;

static long Factorial(int n) => n <= 1 ? 1 : n * Factorial(n - 1);

static int ApplyTwice(Func<int, int> function, int value) => function(function(value));

static double Area(double width, double height) => width * height;

// Try it: write IsPrime(int n) and print all primes up to 30.
