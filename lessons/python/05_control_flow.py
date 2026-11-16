# Lesson 5: Control flow: if, loops and match
#
# Python uses INDENTATION (4 spaces) to show which lines belong together.

temperature = 22
if temperature > 30:
    print("hot")
elif temperature > 15:
    print("pleasant")
else:
    print("cold")

# Comparisons: == != < > <= >=   Logic: and, or, not
weekend = False
if not weekend and temperature > 20:
    print("work day, nice weather")
print(1 < temperature < 30)  # chained comparison

# A conditional expression.
label = "even" if temperature % 2 == 0 else "odd"
print(label)

# Values that count as False: 0, 0.0, "", [], {}, None. Everything else is True.
name = ""
if not name:
    print("name is empty")

# for goes through anything you can loop over.
for i in range(3):          # 0, 1, 2
    print("i =", i)
for i in range(1, 10, 4):   # start, stop (excluded), step: 1, 5, 9
    print("step", i)
for letter in "abc":
    print(letter)
for index, fruit in enumerate(["apple", "pear"], start=1):
    print(index, fruit)

# while repeats while a condition holds.
n = 3
while n > 0:
    print("countdown", n)
    n -= 1

# break leaves a loop, continue skips to the next round, else runs if no break happened.
for i in range(1, 10):
    if i % 2 == 0:
        continue
    if i > 7:
        break
    print("odd", i)

for n in [4, 6, 8]:
    if n % 2 == 1:
        print("found an odd number")
        break
else:
    print("all numbers were even")

# match (Python 3.10+): compare a value to patterns.
command = "go north"
match command.split():
    case ["go", direction]:
        print("going", direction)
    case ["quit"]:
        print("bye")
    case _:
        print("what?")

code = 404
match code:
    case 200:
        print("ok")
    case 404 | 410:
        print("gone")
    case _:
        print("other")

# Try it: print the numbers 1 to 20, but "Fizz" for multiples of 3, "Buzz" for 5, "FizzBuzz" for both.
