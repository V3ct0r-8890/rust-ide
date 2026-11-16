//! The C# project templates (see `templates.rs`). `@NAME@` is the project
//! name, `@IDENT@` the name as a namespace.

const GITIGNORE: &str = "bin/\nobj/\n.vs/\n*.user\n";

const APP_PROPS: &str = r##"<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net10.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
  </PropertyGroup>

</Project>
"##;

pub const CS_SIMPLE: &[(&str, &str)] = &[
    ("@NAME@.csproj", APP_PROPS),
    ("Program.cs", "Console.WriteLine(\"Hello, world!\");\n"),
    (".gitignore", GITIGNORE),
];

pub const CS_LOW: &[(&str, &str)] = &[
    ("@NAME@.csproj", APP_PROPS),
    (
        "Program.cs",
        r##"using @IDENT@;

Console.WriteLine(Greeting.For(args.Length > 0 ? args[0] : "world"));

double[] data = [3, 1, 4, 1, 5, 9, 2, 6];
Console.WriteLine($"mean   = {Stats.Mean(data):F2}");
Console.WriteLine($"median = {Stats.Median(data):F2}");
"##,
    ),
    (
        "Greeting.cs",
        r##"namespace @IDENT@;

public static class Greeting
{
    public static string For(string name) => $"Hello, {name}!";
}
"##,
    ),
    (
        "Stats.cs",
        r##"namespace @IDENT@;

public static class Stats
{
    public static double? Mean(IReadOnlyList<double> numbers) =>
        numbers.Count == 0 ? null : numbers.Sum() / numbers.Count;

    public static double? Median(IReadOnlyList<double> numbers)
    {
        if (numbers.Count == 0)
        {
            return null;
        }
        var sorted = numbers.Order().ToList();
        var middle = sorted.Count / 2;
        return sorted.Count % 2 == 1 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
    }
}
"##,
    ),
    (".gitignore", GITIGNORE),
];

const LIB_PROPS: &str = r##"<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
  </PropertyGroup>

</Project>
"##;

pub const CS_MEDIUM: &[(&str, &str)] = &[
    (
        "@NAME@.slnx",
        r##"<Solution>
  <Project Path="src/@NAME@.Core/@NAME@.Core.csproj" />
  <Project Path="src/@NAME@/@NAME@.csproj" />
  <Project Path="tests/@NAME@.Tests/@NAME@.Tests.csproj" />
</Solution>
"##,
    ),
    ("src/@NAME@.Core/@NAME@.Core.csproj", LIB_PROPS),
    (
        "src/@NAME@.Core/ConfigException.cs",
        r##"namespace @IDENT@.Core;

/// <summary>A mistake in a configuration text.</summary>
public sealed class ConfigException(string message) : Exception(message);
"##,
    ),
    (
        "src/@NAME@.Core/Config.cs",
        r##"namespace @IDENT@.Core;

/// <summary>Settings read from a "key = value" text; "#" starts a comment.</summary>
public sealed record Config(string Greeting = "Hello", int Repeat = 1)
{
    public static Config Parse(string text)
    {
        var config = new Config();
        var number = 0;
        foreach (var raw in text.Split('\n'))
        {
            number++;
            var line = raw.Trim();
            if (line.Length == 0 || line.StartsWith('#'))
            {
                continue;
            }
            var parts = line.Split('=', 2);
            if (parts.Length != 2)
            {
                throw new ConfigException($"line {number}: expected key = value");
            }
            var (key, value) = (parts[0].Trim(), parts[1].Trim());
            switch (key)
            {
                case "greeting":
                    config = config with { Greeting = value };
                    break;
                case "repeat":
                    if (!int.TryParse(value, out var repeat) || repeat < 0)
                    {
                        throw new ConfigException($"line {number}: repeat must be a whole number");
                    }
                    config = config with { Repeat = repeat };
                    break;
                default:
                    throw new ConfigException($"line {number}: unknown key '{key}'");
            }
        }
        return config;
    }

    public static Config Load(string path) => Parse(File.ReadAllText(path));
}
"##,
    ),
    (
        "src/@NAME@.Core/Greeter.cs",
        r##"namespace @IDENT@.Core;

public static class Greeter
{
    /// <summary>The greeting for <paramref name="name"/>, repeated as often as the config says.</summary>
    public static string Greet(string name, Config config) =>
        string.Join('\n', Enumerable.Repeat($"{config.Greeting}, {name}!", config.Repeat));
}
"##,
    ),
    (
        "src/@NAME@/@NAME@.csproj",
        r##"<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net10.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
  </PropertyGroup>

  <ItemGroup>
    <ProjectReference Include="../@NAME@.Core/@NAME@.Core.csproj" />
    <None Include="config.txt" CopyToOutputDirectory="PreserveNewest" />
  </ItemGroup>

</Project>
"##,
    ),
    (
        "src/@NAME@/Program.cs",
        r##"using @IDENT@.Core;

// Usage: dotnet run [name] [config-file]
var name = args.Length > 0 ? args[0] : "world";
var path = args.Length > 1 ? args[1] : Path.Combine(AppContext.BaseDirectory, "config.txt");

try
{
    var config = File.Exists(path) ? Config.Load(path) : new Config();
    Console.WriteLine(Greeter.Greet(name, config));
}
catch (ConfigException e)
{
    Console.Error.WriteLine($"{path}: {e.Message}");
    return 1;
}
return 0;
"##,
    ),
    ("src/@NAME@/config.txt", "# Settings for the greeting\ngreeting = Hello\nrepeat = 2\n"),
    ("tests/@NAME@.Tests/@NAME@.Tests.csproj", TEST_PROJECT_MEDIUM),
    (
        "tests/@NAME@.Tests/ConfigTests.cs",
        r##"using @IDENT@.Core;

namespace @IDENT@.Tests;

public class ConfigTests
{
    [Fact]
    public void ParsesKeysCommentsAndBlankLines()
    {
        var config = Config.Parse("# comment\n\ngreeting = Hi\nrepeat = 3\n");
        Assert.Equal(new Config("Hi", 3), config);
    }

    [Fact]
    public void ReportsTheLineOfAMistake()
    {
        var error = Assert.Throws<ConfigException>(() => Config.Parse("greeting = Hi\nrepeat = many\n"));
        Assert.StartsWith("line 2:", error.Message);
    }
}
"##,
    ),
    (
        "tests/@NAME@.Tests/GreeterTests.cs",
        r##"using @IDENT@.Core;

namespace @IDENT@.Tests;

public class GreeterTests
{
    [Fact]
    public void RepeatsTheGreeting()
    {
        Assert.Equal("Hi, Ann!\nHi, Ann!", Greeter.Greet("Ann", new Config("Hi", 2)));
    }

    [Fact]
    public void ZeroRepeatsGivesNothing()
    {
        Assert.Equal("", Greeter.Greet("Ann", new Config("Hi", 0)));
    }
}
"##,
    ),
    (
        "README.md",
        r##"# @NAME@

A C# solution: a library (`@NAME@.Core`), a console app (`@NAME@`) and xunit tests.

```
dotnet run --project src/@NAME@ -- Ann     # F5 in the IDE
dotnet test                                 # runs the tests
```

Edit `src/@NAME@/config.txt` to change the greeting.
"##,
    ),
    (".gitignore", GITIGNORE),
];

const TEST_PROJECT_MEDIUM: &str = r##"<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
    <IsPackable>false</IsPackable>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="Microsoft.NET.Test.Sdk" Version="17.14.1" />
    <PackageReference Include="xunit" Version="2.9.3" />
    <PackageReference Include="xunit.runner.visualstudio" Version="3.1.4" />
  </ItemGroup>

  <ItemGroup>
    <Using Include="Xunit" />
    <ProjectReference Include="../../src/@NAME@.Core/@NAME@.Core.csproj" />
  </ItemGroup>

</Project>
"##;

pub const CS_COMPLEX: &[(&str, &str)] = &[
    (
        "@NAME@.slnx",
        r##"<Solution>
  <Project Path="src/@NAME@.Util/@NAME@.Util.csproj" />
  <Project Path="src/@NAME@.Core/@NAME@.Core.csproj" />
  <Project Path="src/@NAME@.Cli/@NAME@.Cli.csproj" />
  <Project Path="tests/@NAME@.Tests/@NAME@.Tests.csproj" />
</Solution>
"##,
    ),
    (
        "Directory.Build.props",
        r##"<Project>

  <!-- Settings every project in the solution shares. -->
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
    <LangVersion>latest</LangVersion>
  </PropertyGroup>

</Project>
"##,
    ),
    (
        "src/@NAME@.Util/@NAME@.Util.csproj",
        "<Project Sdk=\"Microsoft.NET.Sdk\">\n\n</Project>\n",
    ),
    (
        "src/@NAME@.Util/TextTools.cs",
        r##"using System.Globalization;
using System.Text.RegularExpressions;

namespace @IDENT@.Util;

public static partial class TextTools
{
    [GeneratedRegex(@"[\p{L}\p{N}']+")]
    private static partial Regex WordPattern();

    /// <summary>The words of a text: runs of letters, digits and apostrophes.</summary>
    public static List<string> Words(string text) => WordPattern().Matches(text).Select(m => m.Value).ToList();

    public static string TitleCase(string text) =>
        CultureInfo.InvariantCulture.TextInfo.ToTitleCase(text.ToLowerInvariant());
}
"##,
    ),
    (
        "src/@NAME@.Core/@NAME@.Core.csproj",
        r##"<Project Sdk="Microsoft.NET.Sdk">

  <ItemGroup>
    <ProjectReference Include="../@NAME@.Util/@NAME@.Util.csproj" />
  </ItemGroup>

</Project>
"##,
    ),
    (
        "src/@NAME@.Core/Analyzer.cs",
        r##"using @IDENT@.Util;

namespace @IDENT@.Core;

public sealed record Summary(int Lines, int Words, string Longest);

public static class Analyzer
{
    public static Summary Summarize(string text)
    {
        var words = TextTools.Words(text);
        var lines = text.Length == 0 ? 0 : text.Split('\n').Length - (text.EndsWith('\n') ? 1 : 0);
        var longest = words.OrderByDescending(w => w.Length).ThenBy(w => w, StringComparer.Ordinal).FirstOrDefault() ?? "";
        return new Summary(lines, words.Count, longest);
    }

    public static string Report(string text)
    {
        var summary = Summarize(text);
        return summary.Words == 0
            ? "no words"
            : $"{summary.Lines} lines, {summary.Words} words, longest: {TextTools.TitleCase(summary.Longest)}";
    }
}
"##,
    ),
    (
        "src/@NAME@.Cli/@NAME@.Cli.csproj",
        r##"<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <OutputType>Exe</OutputType>
  </PropertyGroup>

  <ItemGroup>
    <ProjectReference Include="../@NAME@.Core/@NAME@.Core.csproj" />
    <None Include="../../samples/sample.txt" Link="sample.txt" CopyToOutputDirectory="PreserveNewest" />
  </ItemGroup>

</Project>
"##,
    ),
    (
        "src/@NAME@.Cli/Program.cs",
        r##"using @IDENT@.Core;

// Usage: dotnet run --project src/@NAME@.Cli [text-file]
var path = args.Length > 0 ? args[0] : Path.Combine(AppContext.BaseDirectory, "sample.txt");
if (!File.Exists(path))
{
    Console.Error.WriteLine($"cannot read {path}");
    return 1;
}

Console.WriteLine(Analyzer.Report(File.ReadAllText(path)));
return 0;
"##,
    ),
    (
        "samples/sample.txt",
        "The quick brown fox jumps over the lazy dog.\nA second line, with extraordinary words.\n",
    ),
    ("tests/@NAME@.Tests/@NAME@.Tests.csproj", TEST_PROJECT_COMPLEX),
    (
        "tests/@NAME@.Tests/TextToolsTests.cs",
        r##"using @IDENT@.Util;

namespace @IDENT@.Tests;

public class TextToolsTests
{
    [Fact]
    public void SplitsWords()
    {
        Assert.Equal(["Don't", "panic", "42"], TextTools.Words("Don't panic, 42!"));
    }

    [Fact]
    public void TitleCasesWords()
    {
        Assert.Equal("Hello World", TextTools.TitleCase("hELLO wORLD"));
    }
}
"##,
    ),
    (
        "tests/@NAME@.Tests/AnalyzerTests.cs",
        r##"using @IDENT@.Core;

namespace @IDENT@.Tests;

public class AnalyzerTests
{
    [Fact]
    public void SummarizesAText()
    {
        Assert.Equal(new Summary(2, 5, "three"), Analyzer.Summarize("one two\nthree four five\n"));
    }

    [Fact]
    public void AnEmptyTextHasNoWords()
    {
        Assert.Equal("no words", Analyzer.Report(""));
    }

    [Fact]
    public void TheReportNamesTheLongestWord()
    {
        Assert.Equal("1 lines, 3 words, longest: Elephant", Analyzer.Report("an elephant ate"));
    }
}
"##,
    ),
    (
        ".github/workflows/ci.yml",
        r##"name: CI

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-dotnet@v4
        with:
          dotnet-version: 10.0.x
      - run: dotnet build --nologo
      - run: dotnet test --nologo
"##,
    ),
    (
        "README.md",
        r##"# @NAME@

A C# solution with three layers, shared settings and tests:

| Project | What it holds |
|---|---|
| `@NAME@.Util` | text helpers |
| `@NAME@.Core` | the analysis (uses Util) |
| `@NAME@.Cli` | the console program (uses Core) |
| `@NAME@.Tests` | xunit tests for Util and Core |

```
dotnet run --project src/@NAME@.Cli          # F5 in the IDE
dotnet run --project src/@NAME@.Cli -- file.txt
dotnet test
```

`Directory.Build.props` holds the settings every project shares.
"##,
    ),
    (".gitignore", GITIGNORE),
];

const TEST_PROJECT_COMPLEX: &str = r##"<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <IsPackable>false</IsPackable>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="Microsoft.NET.Test.Sdk" Version="17.14.1" />
    <PackageReference Include="xunit" Version="2.9.3" />
    <PackageReference Include="xunit.runner.visualstudio" Version="3.1.4" />
  </ItemGroup>

  <ItemGroup>
    <Using Include="Xunit" />
    <ProjectReference Include="../../src/@NAME@.Core/@NAME@.Core.csproj" />
  </ItemGroup>

</Project>
"##;
