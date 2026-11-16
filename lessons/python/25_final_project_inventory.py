# Lesson 25: Final project: a small inventory manager
#
# This program uses most of what you learned: classes, dataclasses, dictionaries, exceptions,
# comprehensions, text parsing, type hints, files with JSON, and tests.
# Press F5: it runs a built-in list of commands and then its own tests.

import json
import tempfile
import unittest
from dataclasses import asdict, dataclass
from pathlib import Path


class InventoryError(Exception):
    """Base class for all problems this program reports."""


class UnknownItem(InventoryError):
    def __init__(self, name: str):
        super().__init__(f"we do not stock '{name}'")


class NotEnoughStock(InventoryError):
    def __init__(self, name: str, have: int, wanted: int):
        super().__init__(f"only {have} of {name} left, you asked for {wanted}")


class BadCommand(InventoryError):
    def __init__(self, text: str):
        super().__init__(f"cannot understand: {text}")


@dataclass
class Item:
    name: str
    quantity: int
    price_cents: int


def money(cents: int) -> str:
    return f"${cents // 100}.{cents % 100:02d}"


class Inventory:
    def __init__(self) -> None:
        self.items: dict[str, Item] = {}

    def add(self, name: str, quantity: int, price_cents: int) -> None:
        item = self.items.setdefault(name, Item(name, 0, price_cents))
        item.quantity += quantity
        item.price_cents = price_cents          # the newest price wins

    def remove(self, name: str, quantity: int) -> None:
        item = self.items.get(name)
        if item is None:
            raise UnknownItem(name)
        if item.quantity < quantity:
            raise NotEnoughStock(name, item.quantity, quantity)
        item.quantity -= quantity

    def total_value_cents(self) -> int:
        return sum(i.quantity * i.price_cents for i in self.items.values())

    def low_stock(self, limit: int) -> list[str]:
        return sorted(i.name for i in self.items.values() if i.quantity <= limit)

    def report(self) -> str:
        lines = [f"  {i.name:<8} x{i.quantity:<3} @ {money(i.price_cents)}" for _, i in sorted(self.items.items())]
        lines.append(f"  total value: {money(self.total_value_cents())}")
        return "\n".join(lines)

    def run(self, command: str) -> str:
        """Run one text command such as 'add apple 10 50' or 'remove apple 3'."""
        parts = command.split()
        try:
            match parts:
                case ["add", name, quantity, price]:
                    self.add(name, int(quantity), int(price))
                    return f"added {quantity} {name}"
                case ["remove", name, quantity]:
                    self.remove(name, int(quantity))
                    return f"removed {quantity} {name}"
                case ["report"]:
                    return self.report()
        except ValueError:                       # int() failed
            pass
        raise BadCommand(command)

    def save(self, path: Path) -> None:
        path.write_text(json.dumps([asdict(i) for i in self.items.values()], indent=2))

    @classmethod
    def load(cls, path: Path) -> "Inventory":
        inventory = cls()
        for data in json.loads(path.read_text()):
            item = Item(**data)
            inventory.items[item.name] = item
        return inventory


def demo() -> None:
    inventory = Inventory()
    script = ["add apple 10 50", "add pear 4 80", "add melon 1 450", "remove apple 3",
              "remove pear 9", "remove kiwi 1", "sell apple", "add apple 5 55", "report"]
    for command in script:
        try:
            print(f"> {command}\n  {inventory.run(command)}")
        except InventoryError as error:
            print(f"> {command}\n  ERROR: {error}")
    print("low stock:", inventory.low_stock(4))

    with tempfile.TemporaryDirectory() as folder:
        path = Path(folder) / "inventory.json"
        inventory.save(path)
        print("reloaded equal:", Inventory.load(path).items == inventory.items)


class InventoryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.inventory = Inventory()
        self.inventory.add("apple", 10, 50)

    def test_remove_reduces_stock(self) -> None:
        self.inventory.remove("apple", 4)
        self.assertEqual(self.inventory.items["apple"].quantity, 6)

    def test_cannot_remove_more_than_we_have(self) -> None:
        with self.assertRaises(NotEnoughStock):
            self.inventory.remove("apple", 11)

    def test_unknown_item(self) -> None:
        with self.assertRaises(UnknownItem):
            self.inventory.remove("kiwi", 1)

    def test_total_value(self) -> None:
        self.assertEqual(self.inventory.total_value_cents(), 500)

    def test_bad_command(self) -> None:
        with self.assertRaises(BadCommand):
            self.inventory.run("add apple lots 5")


if __name__ == "__main__":
    demo()
    print()
    unittest.main(argv=["inventory"], exit=False, verbosity=1)

# Ideas to extend it:
#  - read commands from input() in a loop until the user types "quit"
#  - add a --file option with argparse (lesson 21) and load/save automatically
#  - add type checking: install basedpyright and fix what it reports (lesson 18)
#  - show the stock as a table with the string tools of lesson 16
