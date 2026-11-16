// Lesson 17: Nullability
//
// null means "no object". Using it by mistake gives the famous NullReferenceException.
// Modern C# (nullable reference types) helps: a plain `string` should never be null, and
// `string?` says "may be null". The compiler warns when you use a `string?` without checking.

string definitely = "text";
string? maybe = FindName(2);              // may be null

// Console.WriteLine(maybe.Length);       // <- warning: possible null reference. Remove // to see it.

// Check first.
if (maybe != null)
    Console.WriteLine(maybe.Length);
else
    Console.WriteLine("no name found");

// Null-conditional ?. : stops and gives null when the left side is null.
Console.WriteLine(maybe?.ToUpper() ?? "(none)");
Console.WriteLine(FindName(1)?.ToUpper() ?? "(none)");

// Null-coalescing ?? : a default value. ??= : assign only if null.
string display = maybe ?? "anonymous";
Console.WriteLine(display);
string? cache = null;
cache ??= "computed once";
cache ??= "ignored";
Console.WriteLine(cache);

// The ! operator tells the compiler "trust me, it is not null". Use rarely: it hides bugs.
string trusted = FindName(1)!;
Console.WriteLine(trusted.Length);

// Pattern: is not null.
if (FindName(1) is { } name)              // { } matches any non-null value
    Console.WriteLine("found " + name);
if (maybe is null)
    Console.WriteLine("maybe is null");

// Null in collections and with value types.
int? number = null;
Console.WriteLine(number.HasValue + " " + (number ?? 0) + " " + number.GetValueOrDefault(5));
var scores = new Dictionary<string, int> { ["Ada"] = 10 };
int? ada = scores.TryGetValue("Ada", out var s) ? s : null;
int? nobody = scores.TryGetValue("Nobody", out var t) ? t : null;
Console.WriteLine($"{ada} {nobody?.ToString() ?? "no score"}");

// Guard clauses at the start of a method.
Console.WriteLine(Shout("hello"));
try
{
    Shout(null!);
}
catch (ArgumentNullException e)
{
    Console.WriteLine("rejected: " + e.ParamName);
}

// Use records and constructors to make sure required data is always there.
var user = new User("Ada") { Nickname = null };
Console.WriteLine($"{user.Name} {user.Nickname ?? "(no nickname)"}");

// Design tip: return an empty list instead of null, and an Option-like result (TryXxx) when
// "not found" is a normal outcome.
Console.WriteLine(Find(new[] { "a", "b" }, "z").Count);

static string? FindName(int id) => id == 1 ? "Ada" : null;

static string Shout(string text)
{
    ArgumentNullException.ThrowIfNull(text);
    return text.ToUpper() + "!";
}

static List<string> Find(string[] items, string wanted) =>
    items.Where(i => i == wanted).ToList();          // empty, never null

record User(string Name)
{
    public string? Nickname { get; init; }
}

// Try it: turn the warning above into an error by adding <WarningsAsErrors>nullable</WarningsAsErrors> in a project.
