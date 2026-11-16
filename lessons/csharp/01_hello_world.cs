// Lesson 1: Hello, world
//
// Press F5 to run this file (.NET 10 runs a single .cs file directly).
// The output appears in the Output window.
//
// In a modern C# program you can write statements at the top of the file ("top-level statements").
// The program starts at the first line and ends at the last.

Console.WriteLine("Hello, world!");

// {} inside a $"..." string (an "interpolated string") is replaced by the value.
string language = "C#";
Console.WriteLine($"{language} is a modern, type-safe language.");

// Console.Write does not add a new line.
Console.Write("No new line here, ");
Console.WriteLine("but now there is.");

// Placeholders with numbers also work.
Console.WriteLine("My name is {0} and I am {1}.", "Ada", 36);

// Every statement ends with a semicolon. Lines starting with // are comments.
/* A comment can also
   span several lines. */

// Try it: change the text and press F5 again.
// Try it: remove a semicolon and press Alt+F9 to see how an error message looks.
