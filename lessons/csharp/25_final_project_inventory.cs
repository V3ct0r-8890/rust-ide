// Lesson 25: Final project: a small inventory manager
//
// This program uses most of what you learned: records, classes, interfaces, dictionaries, LINQ,
// exceptions, pattern matching, JSON files and small tests.
// Press F5: it runs a built-in list of commands and then its own checks.

#:property PublishAot=false
// The line above lets System.Text.Json use reflection in a single-file program.

using System.Text.Json;

var inventory = new Inventory();
string[] script =
{
    "add apple 10 50", "add pear 4 80", "add melon 1 450", "remove apple 3",
    "remove pear 9", "remove kiwi 1", "sell apple", "add apple 5 55", "report",
};
foreach (var command in script)
{
    try
    {
        Console.WriteLine($"> {command}\n  {inventory.Run(command)}");
    }
    catch (InventoryException e)
    {
        Console.WriteLine($"> {command}\n  ERROR: {e.Message}");
    }
}
Console.WriteLine("low stock: " + string.Join(", ", inventory.LowStock(4)));

// Save and load.
string path = Path.Combine(Path.GetTempPath(), "inventory-lesson25.json");
inventory.Save(path);
var reloaded = Inventory.Load(path);
Console.WriteLine("reloaded equal: " + (reloaded.TotalValueCents() == inventory.TotalValueCents()));
File.Delete(path);

// Self checks.
var test = new Inventory();
test.Add("apple", 10, 50);
Check(test.TotalValueCents() == 500, "total value");
test.Remove("apple", 4);
Check(test.Quantity("apple") == 6, "remove reduces stock");
Check(Throws<NotEnoughStockException>(() => test.Remove("apple", 11)), "cannot remove more than we have");
Check(Throws<UnknownItemException>(() => test.Remove("kiwi", 1)), "unknown item");
Check(Throws<BadCommandException>(() => test.Run("add apple lots 5")), "bad command");

static void Check(bool ok, string name) => Console.WriteLine($"  {(ok ? "PASS" : "FAIL")}  {name}");

static bool Throws<T>(Action action) where T : Exception
{
    try { action(); return false; }
    catch (T) { return true; }
}

static class Fmt
{
    public static string Money(long cents) => $"${cents / 100}.{cents % 100:D2}";
}

record Item(string Name, int Quantity, long PriceCents);

class InventoryException(string message) : Exception(message);
class UnknownItemException(string name) : InventoryException($"we do not stock '{name}'");
class NotEnoughStockException(string name, int have, int wanted)
    : InventoryException($"only {have} of {name} left, you asked for {wanted}");
class BadCommandException(string text) : InventoryException($"cannot understand: {text}");

class Inventory
{
    private Dictionary<string, Item> _items = new();

    public void Add(string name, int quantity, long priceCents)
    {
        int have = _items.TryGetValue(name, out var existing) ? existing.Quantity : 0;
        _items[name] = new Item(name, have + quantity, priceCents);          // the newest price wins
    }

    public void Remove(string name, int quantity)
    {
        if (!_items.TryGetValue(name, out var item)) throw new UnknownItemException(name);
        if (item.Quantity < quantity) throw new NotEnoughStockException(name, item.Quantity, quantity);
        _items[name] = item with { Quantity = item.Quantity - quantity };
    }

    public int Quantity(string name) => _items.TryGetValue(name, out var item) ? item.Quantity : 0;

    public long TotalValueCents() => _items.Values.Sum(i => i.Quantity * i.PriceCents);

    public IEnumerable<string> LowStock(int limit) =>
        _items.Values.Where(i => i.Quantity <= limit).Select(i => i.Name).Order();

    public string Report()
    {
        var lines = _items.Values.OrderBy(i => i.Name)
            .Select(i => $"  {i.Name,-8} x{i.Quantity,-3} @ {Fmt.Money(i.PriceCents)}")
            .Append($"  total value: {Fmt.Money(TotalValueCents())}");
        return string.Join("\n", lines);
    }

    // One text command, for example "add apple 10 50" or "remove apple 3".
    public string Run(string command)
    {
        var parts = command.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        switch (parts)
        {
            case ["add", var name, var q, var price] when int.TryParse(q, out var quantity) && long.TryParse(price, out var cents):
                Add(name, quantity, cents);
                return $"added {quantity} {name}";
            case ["remove", var name, var q] when int.TryParse(q, out var quantity):
                Remove(name, quantity);
                return $"removed {quantity} {name}";
            case ["report"]:
                return Report();
            default:
                throw new BadCommandException(command);
        }
    }

    public void Save(string path) => File.WriteAllText(path, JsonSerializer.Serialize(_items.Values));

    public static Inventory Load(string path)
    {
        var inventory = new Inventory();
        foreach (var item in JsonSerializer.Deserialize<List<Item>>(File.ReadAllText(path))!)
            inventory._items[item.Name] = item;
        return inventory;
    }
}

// Ideas to extend it:
//  - read commands from Console.ReadLine() in a loop until the user types "quit"
//  - make Inventory implement an interface and write a fake for tests (lesson 19)
//  - move it into a project with one class per file (lesson 24) and add xUnit tests (lesson 22)
//  - add a command "sell" that removes stock and records the income
