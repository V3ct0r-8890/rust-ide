# Lesson 7: Lists and tuples

# A list is an ordered collection you can change.
numbers = [5, 3, 8]
numbers.append(1)
numbers.extend([9, 2])
numbers.insert(1, 100)
print(numbers, len(numbers))

last = numbers.pop()        # remove and return the last item
numbers.remove(100)         # remove the first item with that value
print(last, numbers)

# Indexing and slicing work like strings.
print(numbers[0], numbers[-1])
print(numbers[1:3])
numbers[0] = 50             # lists can be changed in place
print(numbers)

# Looking things up.
print(8 in numbers, numbers.index(8), numbers.count(3))

# Sorting.
numbers.sort()
print(numbers)
numbers.sort(reverse=True)
print(numbers)
print(sorted(["pear", "Apple", "fig"], key=str.lower))  # sorted() gives a new list
numbers.reverse()
print(numbers)

# Looping.
for n in numbers:
    print(n, end=" ")
print()
for i, n in enumerate(numbers):
    print(f"#{i} = {n}")

# List comprehension: build a list in one line.
squares = [n * n for n in range(1, 6)]
evens = [n for n in range(10) if n % 2 == 0]
print(squares, evens)

# Useful built-ins.
print(sum(numbers), min(numbers), max(numbers))
print(list(zip(["a", "b", "c"], [1, 2, 3])))

# Copying: assignment does NOT copy.
a = [1, 2, 3]
b = a
b.append(4)
print(a)          # [1, 2, 3, 4]  a and b are the same list
c = a.copy()      # or a[:] or list(a)
c.append(5)
print(a, c)

# A list of lists: a grid.
grid = [[0] * 3 for _ in range(2)]
grid[1][2] = 5
print(grid)

# A tuple is like a list that cannot change. Good for fixed groups of values.
point = (3, 4)
x, y = point            # unpacking
print(x, y, point[0])
single = (1,)           # the comma makes it a tuple
print(type(single))
first, *rest = [1, 2, 3, 4]   # star unpacking
print(first, rest)

# Try it: remove the duplicates from [3, 1, 3, 2, 1] but keep the original order.
