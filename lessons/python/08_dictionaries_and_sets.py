# Lesson 8: Dictionaries and sets

# A dictionary maps keys to values. Keys must be unchangeable (str, int, tuple...).
ages = {"Ada": 36, "Linus": 28}
ages["Grace"] = 45          # add
ages["Ada"] = 37            # replace
print(ages)
print(ages["Ada"])
print(ages.get("Nobody"))       # None instead of an error
print(ages.get("Nobody", 0))    # or a default value
print("Linus" in ages)
del ages["Linus"]
print(len(ages))

# Looping.
for name in ages:
    print(name)
for name, age in ages.items():
    print(f"{name} is {age}")
print(list(ages.keys()), list(ages.values()))

# A missing key raises KeyError.
try:
    print(ages["Nobody"])
except KeyError as error:
    print("no such key:", error)

# Counting words: setdefault / get / Counter.
text = "the cat and the hat and the bat"
counts = {}
for word in text.split():
    counts[word] = counts.get(word, 0) + 1
print(counts)

from collections import Counter, defaultdict
print(Counter(text.split()).most_common(2))

groups = defaultdict(list)  # a missing key starts as an empty list
for word in text.split():
    groups[len(word)].append(word)
print(dict(groups))

# Dictionary comprehension and merging.
squares = {n: n * n for n in range(1, 5)}
print(squares)
merged = {"a": 1} | {"b": 2}
print(merged)

# Dictionaries keep the order in which keys were added.
# Nested data is common (this is what JSON looks like).
person = {"name": "Ada", "languages": ["Python", "Rust"], "address": {"city": "London"}}
print(person["languages"][1], person["address"]["city"])

# A set keeps unique values and has no order.
seen = {3, 1, 3, 2, 1}
print(seen, len(seen))
seen.add(10)
seen.discard(99)  # no error when missing
a, b = {1, 2, 3, 4}, {3, 4, 5}
print(a & b)  # intersection
print(a | b)  # union
print(a - b)  # difference
print(a ^ b)  # in one but not both
print(list(dict.fromkeys([3, 1, 3, 2, 1])))  # remove duplicates, keep order

# Try it: count how often each letter appears in a sentence.
