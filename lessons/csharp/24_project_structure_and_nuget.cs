// Lesson 24: Projects, namespaces and NuGet packages
//
// A single file is fine for learning. A real program is a PROJECT: a folder with a .csproj file
// and many .cs files. Use File > New project in the IDE (or `dotnet new console`) to create one.
//
//   MyApp/
//     MyApp.csproj          settings: .NET version, packages
//     Program.cs            the entry point
//     Models/Person.cs      one class per file, folders match namespaces
//
// A .csproj for a console program looks like this:
//
//   <Project Sdk="Microsoft.NET.Sdk">
//     <PropertyGroup>
//       <OutputType>Exe</OutputType>
//       <TargetFramework>net10.0</TargetFramework>
//       <Nullable>enable</Nullable>
//       <ImplicitUsings>enable</ImplicitUsings>
//     </PropertyGroup>
//     <ItemGroup>
//       <PackageReference Include="Humanizer.Core" Version="2.14.1" />
//     </ItemGroup>
//   </Project>

// A single-file program can also ask for packages with a #: directive (needs an internet connection
// the first time). Remove the // in front of the next two lines to try it:
// #:package Humanizer.Core@2.14.1
// using Humanizer;  // then:  Console.WriteLine(3.Days().Humanize());

// Namespaces group types and avoid name clashes.
using Shapes = Company.Geometry;                  // an alias
using Company.Geometry;
using static System.Math;                         // call Math methods without writing `Math.`

var circle = new Circle(2);
var square = new Company.Geometry.Square(3);
Console.WriteLine($"{circle.Area():F2} {square.Area()}");
Console.WriteLine(Shapes.Measure.Total(circle, square));
Console.WriteLine(Company.Util.Text.Title("hello world"));

// Access modifiers control who can use a member:
//   public     everyone              private   only this class (the default for members)
//   internal   this project          protected this class and its subclasses
Console.WriteLine(new Account().Deposit(50));
// new Account().balance = 1;    // <- error: private

// Extension methods add methods to existing types.
Console.WriteLine("snake_case_name".ToTitle());
Console.WriteLine(new[] { 1, 2, 3, 4 }.EveryOther().Count());

// Static classes hold helpers; `using static` lets you call them without the class name.
Console.WriteLine(Sqrt(16) + Max(3, 4));      // `using static System.Math;` is at the top of the file

// Useful commands (run in a terminal in the project folder):
//   dotnet new console -o MyApp     create a project
//   dotnet run                      build and run
//   dotnet build                    build only
//   dotnet add package <name>       add a NuGet package
//   dotnet test                     run tests
//   dotnet publish -c Release       produce the files to ship

namespace Company.Geometry
{
    public interface IShape { double Area(); }

    public class Circle(double radius) : IShape
    {
        public double Area() => Math.PI * radius * radius;
    }

    public class Square(double side) : IShape
    {
        public double Area() => side * side;
    }

    public static class Measure
    {
        public static double Total(params IShape[] shapes) => Math.Round(shapes.Sum(s => s.Area()), 2);
    }
}

namespace Company.Util
{
    public static class Text
    {
        public static string Title(string s) =>
            string.Join(' ', s.Split(' ').Select(w => char.ToUpper(w[0]) + w[1..]));
    }
}

class Account
{
    private decimal balance;                 // private by default
    public decimal Deposit(decimal amount) => balance += amount;
}

static class Extensions
{
    public static string ToTitle(this string s) => Company.Util.Text.Title(s.Replace('_', ' '));

    public static IEnumerable<T> EveryOther<T>(this IEnumerable<T> items) => items.Where((_, i) => i % 2 == 0);
}

// Try it: create a project with File > New project, add a second .cs file and use a class from it.
