# Lesson 11: Inheritance and polymorphism

class Animal:
    def __init__(self, name):
        self.name = name

    def sound(self):
        return "..."

    def describe(self):
        return f"{self.name} says {self.sound()}"


class Dog(Animal):          # Dog inherits everything from Animal
    def sound(self):        # and replaces one method (overriding)
        return "woof"


class Cat(Animal):
    def __init__(self, name, lives=9):
        super().__init__(name)   # call the parent's __init__
        self.lives = lives

    def sound(self):
        return "meow"

    def describe(self):
        return super().describe() + f" (and has {self.lives} lives)"


# Polymorphism: the same call does the right thing for each type.
for animal in [Dog("Rex"), Cat("Tom"), Animal("Thing")]:
    print(animal.describe())

print(isinstance(Dog("a"), Animal), isinstance(Dog("a"), Cat))
print(issubclass(Cat, Animal))

# Duck typing: Python only cares that the object has the method, not what its class is.
class Robot:
    def sound(self):
        return "beep"


def make_noise(thing):
    print(thing.sound())


make_noise(Dog("x"))
make_noise(Robot())

# Abstract classes force subclasses to implement certain methods.
from abc import ABC, abstractmethod
import math


class Shape(ABC):
    @abstractmethod
    def area(self): ...

    def show(self):
        print(f"{type(self).__name__} with area {self.area():.2f}")


class Circle(Shape):
    def __init__(self, radius):
        self.radius = radius

    def area(self):
        return math.pi * self.radius ** 2


class Square(Shape):
    def __init__(self, side):
        self.side = side

    def area(self):
        return self.side ** 2


for shape in (Circle(1), Square(2)):
    shape.show()

try:
    Shape()  # cannot create an abstract class
except TypeError as error:
    print("error:", error)


# Multiple inheritance and mixins: small classes that add one ability.
class JsonMixin:
    def to_json(self):
        import json
        return json.dumps(self.__dict__)


class User(JsonMixin):
    def __init__(self, name, age):
        self.name, self.age = name, age


print(User("Ada", 36).to_json())
print(Dog.__mro__)  # the order Python searches for methods

# Prefer composition ("has a") over deep inheritance ("is a") when in doubt.
# Try it: add a Bird class whose sound() is "tweet" and add it to the loop above.
