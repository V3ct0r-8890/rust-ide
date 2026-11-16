# Lesson 6: Functions

def add(a, b):
    """Return the sum of a and b. (This text is a docstring: it documents the function.)"""
    return a + b


def greet(name, greeting="Hello"):  # a default value makes the parameter optional
    print(f"{greeting}, {name}!")


print(add(2, 3))
greet("Ada")
greet("Linus", "Hi")
greet(greeting="Welcome", name="Grace")  # keyword arguments: any order


# Return several values: it is really a tuple.
def min_max(numbers):
    return min(numbers), max(numbers)


low, high = min_max([4, 9, -2, 7])
print(low, high)


# *args collects extra positional arguments, **kwargs collects extra keyword arguments.
def total(*numbers):
    return sum(numbers)


def describe(**details):
    for key, value in details.items():
        print(f"  {key} = {value}")


print(total(1, 2, 3, 4))
describe(color="red", size=3)

# A function without return gives None.
print(greet("x") is None)


# Functions are values: pass them around.
def apply_twice(function, value):
    return function(function(value))


def double(x):
    return x * 2


print(apply_twice(double, 3))
print(apply_twice(lambda x: x + 10, 1))  # lambda: a tiny anonymous function

# Recursion: a function that calls itself.
def factorial(n):
    return 1 if n <= 1 else n * factorial(n - 1)


print(factorial(5))

# Scope: variables made inside a function are local to it.
counter = 0


def increase():
    global counter  # needed to CHANGE a module-level variable
    counter += 1


increase()
increase()
print(counter)


# Type hints document what you expect. Python does not enforce them, but editors and
# tools such as basedpyright use them to find mistakes.
def area(width: float, height: float) -> float:
    return width * height


print(area(2.5, 4))

# Beware: a mutable default value is shared between calls.
def bad_append(item, items=[]):
    items.append(item)
    return items


print(bad_append(1), bad_append(2))  # [1, 2] twice: surprising!


def good_append(item, items=None):
    if items is None:
        items = []
    items.append(item)
    return items


print(good_append(1), good_append(2))

# Try it: write is_prime(n) and print all prime numbers up to 30.
