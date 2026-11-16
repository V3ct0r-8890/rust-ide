// Lesson 10: Classes and objects

var rect = new Rectangle(3, 4);
Console.WriteLine($"{rect} area = {rect.Area()}");
rect.Scale(2);
Console.WriteLine(rect);
Console.WriteLine(Rectangle.Square(5));
Console.WriteLine(Rectangle.Created);

var t = new Temperature(100);
Console.WriteLine(t.Fahrenheit);
try
{
    t.Celsius = -300;
}
catch (ArgumentOutOfRangeException e)
{
    Console.WriteLine("rejected: " + e.ParamName);
}

// Object initializers set properties right after creating the object.
var player = new Player { Name = "Ada", Score = 10 };
player.Items.Add("sword");
Console.WriteLine($"{player.Name} {player.Score} {player.Items.Count}");

// Records: classes meant to hold data; they compare by content and print nicely.
var p1 = new Point(1, 2);
var p2 = new Point(1, 2);
Console.WriteLine($"{p1} {p1 == p2} {ReferenceEquals(p1, p2)}");
var moved = p1 with { X = 10 };             // a copy with one change
Console.WriteLine(moved);

// Structs are value types: copied on assignment.
var s1 = new Coordinate(1, 2);
var s2 = s1;
s2.X = 99;
Console.WriteLine($"{s1.X} {s2.X}");

// Classes are reference types: two names for one object.
var r1 = new Rectangle(1, 1);
var r2 = r1;
r2.Scale(10);
Console.WriteLine(r1);

// null: no object.
Rectangle? nothing = null;
Console.WriteLine(nothing?.Area() ?? -1);   // ?. stops when null

// ---- the types ----
class Rectangle
{
    public static int Created;               // shared by all rectangles

    public double Width { get; private set; }  // property: readable by all, settable inside only
    public double Height { get; private set; }

    public Rectangle(double width, double height)
    {
        Width = width;
        Height = height;
        Created++;
    }

    public double Area() => Width * Height;

    public void Scale(double factor)
    {
        Width *= factor;
        Height *= factor;
    }

    public static Rectangle Square(double side) => new(side, side);

    public override string ToString() => $"Rectangle({Width} x {Height})";
}

class Temperature
{
    private double _celsius;

    public Temperature(double celsius) => Celsius = celsius;

    public double Celsius
    {
        get => _celsius;
        set
        {
            if (value < -273.15) throw new ArgumentOutOfRangeException(nameof(value), "below absolute zero");
            _celsius = value;
        }
    }

    public double Fahrenheit => _celsius * 9 / 5 + 32;     // read-only computed property
}

class Player
{
    public required string Name { get; init; }              // must be set when creating
    public int Score { get; set; }
    public List<string> Items { get; } = new();
}

record Point(int X, int Y);

struct Coordinate
{
    public int X, Y;
    public Coordinate(int x, int y) { X = x; Y = y; }
}

// Try it: add a Perimeter() method to Rectangle.
