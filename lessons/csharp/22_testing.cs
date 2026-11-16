// Lesson 22: Testing
//
// In real projects you use a test framework such as xUnit, NUnit or MSTest from a NuGet package:
//
//     public class MathTests
//     {
//         [Fact]
//         public void Clamp_keeps_value_inside() => Assert.Equal(10, MathUtil.Clamp(11, 0, 10));
//     }
//
// and run them with `dotnet test`. Here we write a tiny test runner ourselves, so you can see the
// idea and run it with F5 without downloading anything.

int passed = 0, failed = 0;

void Test(string name, Action body)
{
    try
    {
        body();
        passed++;
        Console.WriteLine($"  PASS  {name}");
    }
    catch (AssertionException e)
    {
        failed++;
        Console.WriteLine($"  FAIL  {name}: {e.Message}");
    }
}

Test("even numbers", () =>
{
    Assert.True(MathUtil.IsEven(2));
    Assert.False(MathUtil.IsEven(3));
    Assert.True(MathUtil.IsEven(-4));
});

Test("clamp keeps the value inside", () =>
{
    Assert.Equal(5, MathUtil.Clamp(5, 0, 10));
    Assert.Equal(0, MathUtil.Clamp(-1, 0, 10));
    Assert.Equal(10, MathUtil.Clamp(11, 0, 10));
});

Test("word count ignores extra spaces", () =>
{
    Assert.Equal(0, TextUtil.WordCount(""));
    Assert.Equal(2, TextUtil.WordCount("  a   b "));
});

Test("divide by zero throws", () =>
{
    Assert.Throws<DivideByZeroException>(() => MathUtil.Divide(1, 0));
});

Test("this test fails on purpose", () =>
{
    Assert.Equal(4, MathUtil.Clamp(100, 0, 10));          // 10 is not 4
});

Console.WriteLine($"\n{passed} passed, {failed} failed (one failure is expected here)");

// ---- code under test ----
static class MathUtil
{
    public static bool IsEven(int n) => n % 2 == 0;
    public static int Clamp(int value, int low, int high) => Math.Max(low, Math.Min(value, high));
    public static int Divide(int a, int b) => a / b;
}

static class TextUtil
{
    public static int WordCount(string text) => text.Split(' ', StringSplitOptions.RemoveEmptyEntries).Length;
}

// ---- a minimal assertion library ----
class AssertionException(string message) : Exception(message);

static class Assert
{
    public static void True(bool condition)
    {
        if (!condition) throw new AssertionException("expected true");
    }

    public static void False(bool condition)
    {
        if (condition) throw new AssertionException("expected false");
    }

    public static void Equal<T>(T expected, T actual)
    {
        if (!EqualityComparer<T>.Default.Equals(expected, actual))
            throw new AssertionException($"expected {expected}, but got {actual}");
    }

    public static void Throws<TException>(Action action) where TException : Exception
    {
        try { action(); }
        catch (TException) { return; }
        throw new AssertionException($"expected {typeof(TException).Name}");
    }
}

// Tips:
//  - One test per behavior, with names that read like sentences.
//  - Arrange (prepare), Act (call), Assert (check).
//  - Use interfaces (lesson 19) so fakes can replace databases, clocks and web services.
//  - Try it: add a test for a new method `Double(int n)`; first make it fail, then pass.
