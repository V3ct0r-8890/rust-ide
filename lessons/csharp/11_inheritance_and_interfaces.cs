// Lesson 11: Inheritance, interfaces and polymorphism

Animal[] animals = { new Dog("Rex"), new Cat("Tom"), new Animal("Thing") };
foreach (var animal in animals)
    Console.WriteLine(animal.Describe());

Console.WriteLine(animals[0] is Dog);
Console.WriteLine(animals[0] is Cat);
if (animals[1] is Cat cat)
    Console.WriteLine($"{cat.Name} has {cat.Lives} lives");

// Interfaces: a contract. Any type that implements it can be used through it.
var shapes = new List<IShape> { new Circle(1), new Square(2), new Circle(0.5) };
foreach (var shape in shapes)
    Console.WriteLine($"{shape.GetType().Name}: {shape.Area():F2}");
Console.WriteLine($"total area {shapes.Sum(s => s.Area()):F2}");

// A class can implement several interfaces but inherit from only one class.
var dog = new Robot();
dog.Speak();
dog.Charge();

// abstract classes cannot be created directly.
Employee[] staff = { new Manager("Grace", 5000), new Developer("Linus", 4000) };
foreach (var e in staff)
    Console.WriteLine($"{e.Name} earns {e.Pay()}");

// Interface default method and a generic helper that accepts any IShape.
Console.WriteLine(Largest(shapes).Describe());

static IShape Largest(IEnumerable<IShape> shapes) => shapes.MaxBy(s => s.Area())!;

// ---- the types ----
class Animal
{
    public string Name { get; }
    public Animal(string name) => Name = name;

    public virtual string Sound() => "...";              // virtual: may be replaced
    public string Describe() => $"{Name} says {Sound()}";
}

class Dog : Animal
{
    public Dog(string name) : base(name) { }
    public override string Sound() => "woof";
}

class Cat : Animal
{
    public int Lives { get; } = 9;
    public Cat(string name) : base(name) { }
    public override string Sound() => "meow";
}

interface IShape
{
    double Area();
    string Describe() => $"a {GetType().Name} with area {Area():F2}";    // default implementation
}

class Circle(double radius) : IShape        // primary constructor (C# 12)
{
    public double Area() => Math.PI * radius * radius;
}

class Square(double side) : IShape
{
    public double Area() => side * side;
}

interface ISpeaker { void Speak(); }
interface IChargeable { void Charge(); }

class Robot : ISpeaker, IChargeable
{
    public void Speak() => Console.WriteLine("beep");
    public void Charge() => Console.WriteLine("charging");
}

abstract class Employee
{
    public string Name { get; }
    protected decimal BaseSalary { get; }

    protected Employee(string name, decimal baseSalary)
    {
        Name = name;
        BaseSalary = baseSalary;
    }

    public abstract decimal Pay();           // subclasses MUST implement this
}

class Manager(string name, decimal salary) : Employee(name, salary)
{
    public override decimal Pay() => BaseSalary + 1000;
}

class Developer(string name, decimal salary) : Employee(name, salary)
{
    public override decimal Pay() => BaseSalary + 500;
}

// Prefer composition ("has a") over deep inheritance ("is a") when in doubt.
// Try it: add a Bird class that says "tweet" and add it to the array.
