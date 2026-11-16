// Lesson 4: Strings
//
// Strings cannot be changed. Every operation returns a NEW string.

using System.Text;

string text = "  The quick brown fox  ";
Console.WriteLine($"[{text.Trim()}]");
Console.WriteLine(text.ToUpper());
Console.WriteLine(text.ToLower());
Console.WriteLine(text.Length);
Console.WriteLine(text.Contains("quick"));
Console.WriteLine(text.Replace("quick", "slow"));
Console.WriteLine(text.IndexOf("brown"));
Console.WriteLine(text.Trim().StartsWith("The") + " " + text.Trim().EndsWith("x"));

// Indexing and slicing (ranges).
string word = "Python";
Console.WriteLine(word[0]);
Console.WriteLine(word[^1]);          // ^1 is the last character
Console.WriteLine(word.Substring(0, 2));
Console.WriteLine(word[2..]);         // "thon"
Console.WriteLine(word[..3]);         // "Pyt"
Console.WriteLine(new string(word.Reverse().ToArray()));

// Splitting and joining.
string csv = "red,green,blue";
string[] parts = csv.Split(',');
Console.WriteLine(parts.Length);
Console.WriteLine(string.Join(" + ", parts));
foreach (var w in "one two  three".Split(' ', StringSplitOptions.RemoveEmptyEntries))
    Console.WriteLine("word: " + w);

// Building text.
string name = "Ada";
double score = 9.5;
Console.WriteLine($"{name} scored {score:F1} points");
Console.WriteLine(name + " " + score);
Console.WriteLine(string.Format("{0} / {1}", name, score));
Console.WriteLine(new string('-', 20));

// A StringBuilder is faster when you add text many times.
var builder = new StringBuilder();
for (int i = 1; i <= 5; i++)
    builder.Append(i).Append(i < 5 ? ", " : "");
builder.AppendLine();
builder.Append("done");
Console.WriteLine(builder.ToString());

// Escape sequences, verbatim strings and raw string literals.
Console.WriteLine("Tab:\there\nNew line above and a \"quote\"");
Console.WriteLine(@"C:\new\table");
Console.WriteLine("""
    A raw string literal keeps "quotes" and \backslashes\ as they are.
      The indentation of the closing quotes is removed.
    """);

// Comparing.
Console.WriteLine("apple" == "apple");
Console.WriteLine(string.Equals("ABC", "abc", StringComparison.OrdinalIgnoreCase));
Console.WriteLine(string.Compare("apple", "banana"));   // negative: apple comes first

// Characters.
Console.WriteLine(char.IsDigit('7') + " " + char.IsLetter('x') + " " + char.ToUpper('q'));
Console.WriteLine((int)'A' + " " + (char)97);

// Checks and padding.
Console.WriteLine(string.IsNullOrEmpty("") + " " + string.IsNullOrWhiteSpace("  "));
Console.WriteLine("7".PadLeft(3, '0') + " " + "ab".PadRight(5, '.') + "|");

// Unicode works.
string thai = "สวัสดี";
Console.WriteLine($"{thai} {thai.Length}");

// Try it: write a method that tells whether a word is a palindrome.
