// Lesson 23: Command-line arguments, environment variables and exit codes
//
// In top-level programs the arguments are available in the variable `args`.
// From a terminal:   dotnet run 23_args_and_environment.cs -- -v -n 2 Ada Linus

using System.Diagnostics;
using System.Runtime.InteropServices;

Console.WriteLine("arguments: [" + string.Join(", ", args) + "]");

// A small parser. It takes a plain array, so it is easy to test.
foreach (var example in new[] { new[] { "-v", "-n", "2", "Ada", "Linus" }, new[] { "--oops" }, new[] { "-n" }, new string[0] })
{
    try
    {
        Console.WriteLine($"[{string.Join(" ", example)}] -> {Options.Parse(example)}");
    }
    catch (ArgumentException e)
    {
        Console.WriteLine($"[{string.Join(" ", example)}] -> error: {e.Message}");
    }
}

Options options;
try
{
    options = Options.Parse(args);
}
catch (ArgumentException e)
{
    Console.Error.WriteLine("error: " + e.Message);      // the error stream
    return 2;                                             // non-zero exit code means failure
}

for (int i = 0; i < options.Count; i++)
    foreach (var name in options.Names)
        Console.WriteLine($"hello {name}");

// Environment variables.
Console.WriteLine(Environment.GetEnvironmentVariable("HOME") ?? Environment.GetEnvironmentVariable("USERPROFILE"));
Console.WriteLine(Environment.GetEnvironmentVariable("LESSON_LEVEL") ?? "default");
Environment.SetEnvironmentVariable("MY_SETTING", "on");          // for this process and its children
Console.WriteLine(Environment.GetEnvironmentVariable("MY_SETTING"));

// Information about the machine.
Console.WriteLine($"{RuntimeInformation.OSDescription.Length > 0} {RuntimeInformation.ProcessArchitecture}");
Console.WriteLine(Directory.GetCurrentDirectory().Length > 0);
Console.WriteLine(Environment.NewLine.Length > 0);
Console.WriteLine($".NET {Environment.Version.Major}");

// Running another program.
var psi = new ProcessStartInfo("dotnet", "--version") { RedirectStandardOutput = true, UseShellExecute = false };
using (var process = Process.Start(psi)!)
{
    string version = process.StandardOutput.ReadToEnd().Trim();
    process.WaitForExit();
    Console.WriteLine($"dotnet {version} exited with {process.ExitCode}");
}

// Returning a number from top-level code sets the exit code. 0 = success.
return 0;

record Options(bool Verbose, int Count, List<string> Names)
{
    public static Options Parse(string[] args)
    {
        bool verbose = false;
        int count = 1;
        var names = new List<string>();
        for (int i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "-v" or "--verbose":
                    verbose = true;
                    break;
                case "-n" or "--count":
                    if (++i >= args.Length) throw new ArgumentException("--count needs a number");
                    if (!int.TryParse(args[i], out count)) throw new ArgumentException($"'{args[i]}' is not a number");
                    break;
                case var flag when flag.StartsWith('-'):
                    throw new ArgumentException($"unknown option {flag}");
                default:
                    names.Add(args[i]);
                    break;
            }
        }
        return new Options(verbose, count, names);
    }

    public override string ToString() => $"verbose={Verbose}, count={Count}, names=[{string.Join(", ", Names)}]";
}

// Try it: add a --shout option that prints the names in upper case.
