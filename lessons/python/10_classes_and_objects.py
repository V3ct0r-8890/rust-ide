# Lesson 10: Classes and objects

class Rectangle:
    """A rectangle with a width and a height."""

    sides = 4  # a class attribute: shared by every rectangle

    def __init__(self, width, height):  # runs when you write Rectangle(...)
        self.width = width              # instance attributes: belong to one object
        self.height = height

    def area(self):                     # a method: the first parameter is always self
        return self.width * self.height

    def scale(self, factor):
        self.width *= factor
        self.height *= factor

    def __str__(self):                  # used by print() and str()
        return f"Rectangle({self.width} x {self.height})"

    def __repr__(self):                 # used in lists and the debugger
        return f"Rectangle({self.width!r}, {self.height!r})"

    def __eq__(self, other):            # makes == compare the contents
        return isinstance(other, Rectangle) and (self.width, self.height) == (other.width, other.height)

    def __lt__(self, other):            # makes < and sorted() work
        return self.area() < other.area()


r = Rectangle(3, 4)
print(r, r.area())
r.scale(2)
print(r)
print([Rectangle(1, 2), Rectangle(3, 3)])
print(Rectangle(2, 3) == Rectangle(2, 3))
print(sorted([Rectangle(5, 5), Rectangle(1, 1), Rectangle(2, 3)]))
print(Rectangle.sides, r.sides)


# Properties: a method that looks like an attribute, with validation.
class Temperature:
    def __init__(self, celsius):
        self.celsius = celsius

    @property
    def celsius(self):
        return self._celsius

    @celsius.setter
    def celsius(self, value):
        if value < -273.15:
            raise ValueError("below absolute zero")
        self._celsius = value

    @property
    def fahrenheit(self):
        return self._celsius * 9 / 5 + 32


t = Temperature(100)
print(t.fahrenheit)
try:
    t.celsius = -300
except ValueError as error:
    print("rejected:", error)


# Static and class methods.
class Circle:
    count = 0

    def __init__(self, radius):
        self.radius = radius
        Circle.count += 1

    @staticmethod
    def pi():
        return 3.14159

    @classmethod
    def unit(cls):          # an alternative constructor
        return cls(1)


print(Circle.pi(), Circle.unit().radius, Circle.count)

# Dataclasses write __init__, __repr__ and __eq__ for you.
from dataclasses import dataclass, field


@dataclass
class Player:
    name: str
    score: int = 0
    items: list = field(default_factory=list)


p = Player("Ada")
p.score += 10
p.items.append("sword")
print(p, p == Player("Ada", 10, ["sword"]))

# By convention a name starting with _ is "private": please do not touch it from outside.

# Try it: add a perimeter() method to Rectangle.
