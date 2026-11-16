# Lesson 24: Pythonic code, and a little about speed

import timeit

# --- Idioms: the Python way of saying common things ---
items = ["a", "b", "c"]

# Loop with the index and the value together.
for i, item in enumerate(items):
    print(i, item)

# Loop over two lists together.
for a, b in zip(items, [1, 2, 3]):
    print(a, b)

# Swap, chain compare, and unpack.
x, y = 1, 2
x, y = y, x
print(0 < x < 10)
first, *middle, last = [1, 2, 3, 4, 5]
print(first, middle, last)

# Truthiness: write `if items:` instead of `if len(items) > 0:`.
if items:
    print("not empty")

# Use `is None`, not `== None`.
value = None
print(value is None)

# Build strings with join, not += in a loop.
print(", ".join(str(n) for n in range(5)))

# `with` for anything that needs closing. EAFP: try first, ask forgiveness later.
config = {"debug": True}
try:
    print(config["verbose"])
except KeyError:
    print("verbose not set")
print(config.get("verbose", False))

# The walrus operator := assigns inside an expression.
if (n := len(items)) > 2:
    print(f"{n} items is more than 2")

# Sorting with several keys, and `max` with a key.
people = [("Ada", 36), ("Linus", 36), ("Grace", 45)]
print(sorted(people, key=lambda p: (-p[1], p[0])))
print(max(people, key=lambda p: p[1]))

# Default dicts, Counters and sets save loops.
from collections import Counter
print(Counter("mississippi").most_common(2))

# --- Measuring speed: never guess, measure ---
def with_loop(n):
    result = []
    for i in range(n):
        result.append(i * i)
    return result


def with_comprehension(n):
    return [i * i for i in range(n)]


loop_time = timeit.timeit(lambda: with_loop(1000), number=200)
comp_time = timeit.timeit(lambda: with_comprehension(1000), number=200)
print(f"loop {loop_time:.4f}s, comprehension {comp_time:.4f}s")

# Membership test: list is O(n), set is O(1).
big_list = list(range(10_000))
big_set = set(big_list)
print("list:", round(timeit.timeit(lambda: 9_999 in big_list, number=500), 5))
print("set: ", round(timeit.timeit(lambda: 9_999 in big_set, number=500), 5))

# Slow string building vs join.
def concat(n):
    s = ""
    for i in range(n):
        s += str(i)
    return s


def joined(n):
    return "".join(str(i) for i in range(n))


print(concat(100) == joined(100))

# Profile a whole program with: python -m cProfile -s cumtime yourfile.py
# Rule of thumb: make it correct, make it clear, and only then make it fast (where you measured).
# Try it: time sum(range(n)) against a hand-written loop for n = 1_000_000.
