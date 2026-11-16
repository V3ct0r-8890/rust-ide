# Lesson 18: Type hints
#
# Type hints tell readers and tools what kind of values to expect. Python itself ignores them
# when running, but the IDE's language server (basedpyright) uses them to show errors while you type.
# Try it: change a call below to pass the wrong type and look at the error markers.

from dataclasses import dataclass
from typing import Callable, Optional, Protocol, TypedDict, Union


def greet(name: str, times: int = 1) -> str:
    return ("Hello, " + name + "! ") * times


print(greet("Ada", 2))

# Collections: list[int], dict[str, float], tuple[int, str], set[str].
def average(numbers: list[float]) -> float:
    return sum(numbers) / len(numbers)


def invert(mapping: dict[str, int]) -> dict[int, str]:
    return {value: key for key, value in mapping.items()}


print(average([1, 2, 3.5]), invert({"a": 1, "b": 2}))

# None-able values: X | None (older code writes Optional[X]).
def find_user(users: dict[int, str], user_id: int) -> str | None:
    return users.get(user_id)


name = find_user({1: "Ada"}, 2)
if name is not None:          # the checker makes you test before using
    print(name.upper())
else:
    print("no such user")

# Unions and literals.
def to_number(value: int | str) -> int:
    return int(value)


print(to_number("7") + to_number(3))

from typing import Literal
def move(direction: Literal["left", "right"]) -> str:
    return f"moving {direction}"


print(move("left"))

# Functions as values.
def apply(function: Callable[[int], int], value: int) -> int:
    return function(value)


print(apply(lambda x: x + 1, 4))

# Structured data.
class Movie(TypedDict):
    title: str
    year: int


movie: Movie = {"title": "Alien", "year": 1979}
print(movie["title"])


@dataclass
class Point:
    x: float
    y: float

    def distance_to(self, other: "Point") -> float:
        return ((self.x - other.x) ** 2 + (self.y - other.y) ** 2) ** 0.5


print(Point(0, 0).distance_to(Point(3, 4)))


# Protocol: "anything with these methods" (structural typing, like duck typing but checked).
class Speaker(Protocol):
    def speak(self) -> str: ...


class Dog:
    def speak(self) -> str:
        return "woof"


def talk(s: Speaker) -> None:
    print(s.speak())


talk(Dog())


# Generics.
from typing import TypeVar
T = TypeVar("T")


def first(items: list[T]) -> T:
    return items[0]


print(first([10, 20]), first(["a", "b"]))

# Hints are optional. Add them to function signatures first: that gives the biggest benefit.
# Try it: add hints to the functions you wrote in earlier lessons.
