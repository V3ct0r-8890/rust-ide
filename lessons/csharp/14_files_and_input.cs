// Lesson 14: Input and files
//
// Press F5. When the program asks a question, type the answer into the input line of the
// Output window and press Enter (or just press Enter to accept the default).

#:property PublishAot=false
// The line above lets System.Text.Json use reflection in a single-file program.

using System.Text.Json;

Console.Write("What is your name? ");
string? input = Console.ReadLine();                     // null when there is no input
string name = string.IsNullOrWhiteSpace(input) ? "friend" : input.Trim();
Console.WriteLine($"Hello, {name}!");

// ReadLine always gives text: convert it yourself, e.g. int.TryParse(...)

// Work in a temporary folder so we never touch your real files.
string folder = Path.Combine(Path.GetTempPath(), "csharp-lesson14-" + Guid.NewGuid().ToString("N")[..8]);
Directory.CreateDirectory(folder);
string file = Path.Combine(folder, "notes.txt");

// Write the whole file, append, read everything.
File.WriteAllText(file, "first line\nsecond line\n");
File.AppendAllText(file, $"third line for {name}\n");
string text = File.ReadAllText(file);
Console.WriteLine($"--- {Path.GetFileName(file)} ---\n{text}---");

// Line by line.
int number = 1;
foreach (string line in File.ReadLines(file))
    Console.WriteLine($"{number++,2}: {line}");
Console.WriteLine(File.ReadAllLines(file).Length);

// Streams: using closes the file when the block ends.
using (var writer = new StreamWriter(Path.Combine(folder, "log.txt")))
{
    writer.WriteLine("line A");
    writer.WriteLine("line B");
}
using (var reader = new StreamReader(Path.Combine(folder, "log.txt")))
{
    string? line;
    while ((line = reader.ReadLine()) != null)
        Console.WriteLine("read: " + line);
}

// Information about files and folders.
var info = new FileInfo(file);
Console.WriteLine($"{info.Exists} {info.Extension} {info.Length} bytes {Path.GetFileNameWithoutExtension(file)}");
File.WriteAllText(Path.Combine(folder, "other.txt"), "x");
var names = Directory.GetFiles(folder).Select(Path.GetFileName).Order();
Console.WriteLine(string.Join(", ", names));
Console.WriteLine(string.Join(", ", Directory.GetFiles(folder, "*.txt").Length));

// JSON.
var people = new List<Person> { new("Ada", 36), new("Linus", 28) };
string json = JsonSerializer.Serialize(people, new JsonSerializerOptions { WriteIndented = true });
File.WriteAllText(Path.Combine(folder, "people.json"), json);
var loaded = JsonSerializer.Deserialize<List<Person>>(File.ReadAllText(Path.Combine(folder, "people.json")))!;
Console.WriteLine(json);
Console.WriteLine(loaded[1].Name);

// Always expect missing files.
Console.WriteLine(File.Exists(Path.Combine(folder, "missing.txt")));
try
{
    File.ReadAllText(Path.Combine(folder, "missing.txt"));
}
catch (FileNotFoundException)
{
    Console.WriteLine("missing.txt does not exist");
}

// Clean up.
Directory.Delete(folder, recursive: true);
Console.WriteLine("cleaned up: " + !Directory.Exists(folder));

record Person(string Name, int Age);

// Try it: write a program that counts the lines and words of a text file.
