# Lesson 15: Closures, decorators and functional tools

import functools
import time


# A closure: an inner function that remembers variables of the outer function.
def make_counter():
    count = 0

    def counter():
        nonlocal count  # we want to change the outer variable
        count += 1
        return count

    return counter


a, b = make_counter(), make_counter()
print(a(), a(), a(), b())


def make_multiplier(factor):
    return lambda x: x * factor


triple = make_multiplier(3)
print(triple(5))


# A decorator is a function that takes a function and returns an improved one.
def timed(function):
    @functools.wraps(function)        # keeps the original name and docstring
    def wrapper(*args, **kwargs):
        start = time.perf_counter()
        result = function(*args, **kwargs)
        elapsed = time.perf_counter() - start
        print(f"{function.__name__} took {elapsed * 1000:.2f} ms")
        return result
    return wrapper


@timed                                # same as: slow_sum = timed(slow_sum)
def slow_sum(n):
    """Add the numbers below n."""
    return sum(range(n))


print(slow_sum(100_000))
print(slow_sum.__name__)


# A decorator with its own parameter needs one more level.
def repeat(times):
    def decorator(function):
        @functools.wraps(function)
        def wrapper(*args, **kwargs):
            return [function(*args, **kwargs) for _ in range(times)]
        return wrapper
    return decorator


@repeat(3)
def hello(name):
    return f"hello {name}"


print(hello("Ada"))


# Caching results: lru_cache turns a slow recursive function into a fast one.
@functools.lru_cache(maxsize=None)
def fib(n):
    return n if n < 2 else fib(n - 1) + fib(n - 2)


print(fib(80))
print(fib.cache_info().hits > 0)

# functools helpers.
from functools import partial, reduce
power_of_two = partial(pow, 2)        # fixes the first argument
print(power_of_two(10))
print(reduce(lambda acc, n: acc * n, range(1, 6)))

# Sorting by a key function.
words = ["banana", "fig", "apple", "kiwi"]
print(sorted(words, key=len))
print(sorted(words, key=lambda w: (len(w), w)))

# Context managers: the `with` statement. Write your own with a class or a generator.
from contextlib import contextmanager


@contextmanager
def announce(label):
    print(f"start {label}")
    try:
        yield
    finally:
        print(f"end {label}")


with announce("work"):
    print("  working")

# Try it: write a decorator that prints the arguments every time the function is called.
