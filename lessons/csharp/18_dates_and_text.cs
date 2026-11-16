// Lesson 18: Dates, times and regular expressions

using System.Globalization;
using System.Text.RegularExpressions;

// --- Dates (fixed values, so the output is always the same) ---
var birthday = new DateOnly(1990, 6, 15);
var meeting = new DateTime(2026, 3, 14, 9, 30, 0);
Console.WriteLine($"{birthday} {meeting}");
Console.WriteLine(birthday.DayOfWeek);
Console.WriteLine(meeting.ToString("dddd, dd MMMM yyyy HH:mm", CultureInfo.InvariantCulture));

// Arithmetic.
Console.WriteLine(meeting.AddDays(7));
Console.WriteLine(meeting - TimeSpan.FromHours(10.75));
var left = new DateOnly(2026, 12, 25).DayNumber - new DateOnly(2026, 3, 14).DayNumber;
Console.WriteLine($"{left} days");
Console.WriteLine(new DateTime(2026, 12, 25) - new DateTime(2026, 3, 14));      // a TimeSpan

// Parsing. Always pass a culture so results do not depend on the computer's settings.
var parsed = DateTime.ParseExact("2026-03-14 09:30", "yyyy-MM-dd HH:mm", CultureInfo.InvariantCulture);
Console.WriteLine(parsed == meeting);
Console.WriteLine(DateTime.TryParse("not a date", out _));
Console.WriteLine(meeting.ToString("o"));        // ISO 8601, round-trip format

// UTC and offsets.
var utc = new DateTimeOffset(2026, 3, 14, 9, 30, 0, TimeSpan.Zero);
Console.WriteLine(utc + " -> " + utc.ToOffset(TimeSpan.FromHours(7)));
Console.WriteLine(DateTime.Now.Year >= 2026);   // "now" changes every run, so we only test it

Console.WriteLine(DateTime.IsLeapYear(2028) + " " + DateTime.DaysInMonth(2026, 2));

// --- Regular expressions ---
string text = "Contact: ada@example.com, linus@kernel.org or call 555-1234 on 2026-03-14.";

Console.WriteLine(Regex.Match(text, @"\d+").Value);
foreach (Match m in Regex.Matches(text, @"\d{4}-\d{2}-\d{2}"))
    Console.WriteLine("date: " + m.Value);
Console.WriteLine(string.Join(" | ", Regex.Matches(text, @"[\w.]+@[\w.]+\w").Select(m => m.Value)));
Console.WriteLine(Regex.Replace("call 555-1234", @"\d", "#"));
Console.WriteLine(Regex.IsMatch("555-1234", @"^\d{3}-\d{4}$"));
Console.WriteLine(string.Join("|", Regex.Split("a, b;c,  d", @"[,;]\s*")));

// Named groups.
var match = Regex.Match(text, @"(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})");
Console.WriteLine($"{match.Groups["year"].Value}/{match.Groups["month"].Value}/{match.Groups["day"].Value}");

// Replace with a function.
Console.WriteLine(Regex.Replace("3 apples and 10 pears", @"\d+", m => (int.Parse(m.Value) * 2).ToString()));

// Cheat sheet:  . any   \d digit   \w letter/digit/_   \s space
//   * zero or more   + one or more   ? optional   {2,4} two to four   ^ start   $ end   [abc] (a|b)

// Compiled regexes created at build time (fast): [GeneratedRegex] on a partial method. See the docs.

// Try it: check that a password has at least 8 characters, a digit and an upper-case letter.
