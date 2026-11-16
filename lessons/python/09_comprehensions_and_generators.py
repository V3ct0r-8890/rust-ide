# Lesson 9: Comprehensions, generators and iteration tools

numbers = list(range(1, 11))

# Comprehensions build collections from a loop in one expression.
print([n * n for n in numbers if n % 2 == 1])           # list
print({n: n % 3 for n in numbers[:5]})                   # dict
print({n % 4 for n in numbers})                          # set
pairs = [(x, y) for x in range(1, 4) for y in range(1, 4) if x < y]
print(pairs)

# map, filter and friends.
print(list(map(str.upper, ["a", "b"])))
print(list(filter(lambda n: n > 7, numbers)))
print(any(n > 9 for n in numbers), all(n > 0 for n in numbers))
print(sum(n * n for n in numbers))   # a generator expression: no list in memory
print(max(["pear", "fig", "banana"], key=len))

# A generator produces values one at a time with `yield`. It remembers where it stopped.
def countdown(start):
    while start > 0:
        yield start
        start -= 1

for n in countdown(3):
    print("count", n)

def fibonacci():
    a, b = 0, 1
    while True:          # infinite, but only computes what is asked for
        yield a
        a, b = b, a + b

from itertools import islice, chain, groupby, product, permutations, accumulate
print(list(islice(fibonacci(), 10)))

# itertools has many building blocks.
print(list(chain([1, 2], [3], [4, 5])))
print(list(accumulate([1, 2, 3, 4])))
print(list(product("ab", [1, 2])))
print(list(permutations([1, 2, 3], 2)))
for key, group in groupby(["apple", "avocado", "banana", "blueberry", "cherry"], key=lambda w: w[0]):
    print(key, list(group))

# zip, enumerate, reversed, sorted.
names = ["Ada", "Linus", "Grace"]
ages = [36, 28, 45]
for name, age in zip(names, ages):
    print(name, age)
print(dict(zip(names, ages)))
print(list(reversed(names)))
print(sorted(zip(ages, names)))

# Iterators: anything with a __next__ method; next() asks for one item.
it = iter([10, 20])
print(next(it), next(it), next(it, "done"))

# Try it: write a generator that yields the prime numbers one by one.
