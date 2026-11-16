// Lesson 3: Numbers and math

int a = 17, b = 5;
Console.WriteLine($"{a} + {b} = {a + b}");
Console.WriteLine($"{a} - {b} = {a - b}");
Console.WriteLine($"{a} * {b} = {a * b}");
Console.WriteLine($"{a} / {b} = {a / b}      (two ints divide without a fraction)");
Console.WriteLine($"{a} % {b} = {a % b}      (the remainder)");
Console.WriteLine($"{(double)a / b}          (cast one side to keep the fraction)");
Console.WriteLine(17.0 / 5);

// Floating point numbers are approximate; decimal is exact for money.
Console.WriteLine(0.1 + 0.2 == 0.3);
Console.WriteLine(0.1m + 0.2m == 0.3m);
Console.WriteLine(Math.Abs(0.1 + 0.2 - 0.3) < 1e-9);

// The Math class.
Console.WriteLine(Math.Sqrt(2));
Console.WriteLine(Math.Pow(2, 10));
Console.WriteLine(Math.Abs(-8));
Console.WriteLine(Math.Max(3, 9) + " " + Math.Min(3, 9));
Console.WriteLine($"{Math.Floor(3.7)} {Math.Ceiling(3.2)} {Math.Round(3.14159, 2)}");
Console.WriteLine($"{Math.Round(2.5)} {Math.Round(3.5)} {Math.Round(2.5, MidpointRounding.AwayFromZero)}");
Console.WriteLine(Math.PI);
Console.WriteLine(Math.Clamp(15, 0, 10));

// Overflow: unchecked by default (wraps around); `checked` makes it an error.
int max = int.MaxValue;
Console.WriteLine(unchecked(max + 1));
try
{
    Console.WriteLine(checked(max + 1));
}
catch (OverflowException)
{
    Console.WriteLine("overflow detected");
}

// Compound operators and increments.
int n = 5;
n *= 2;
n -= 3;
Console.WriteLine(n++);   // prints 7, then n becomes 8
Console.WriteLine(++n);   // n becomes 9, then prints 9

// Formatting numbers.
double value = 1234567.891;
Console.WriteLine(value.ToString("F2"));       // two decimals
Console.WriteLine(value.ToString("N1"));       // thousands separator
Console.WriteLine(42.ToString("D5"));          // zero padded
Console.WriteLine(0.256.ToString("P1"));       // percentage
Console.WriteLine(255.ToString("X"));          // hex
Console.WriteLine($"[{42,6}] [{42,-6}] [{value:C0}]");   // width, left aligned, currency

// Random numbers (seeded here so the result is repeatable).
var random = new Random(7);
Console.WriteLine(random.Next(1, 7) >= 1);

// Large numbers.
System.Numerics.BigInteger factorial = 1;
for (int i = 1; i <= 25; i++) factorial *= i;
Console.WriteLine(factorial);

// Try it: compute the area of a circle with radius 3 and print it with two decimals.
