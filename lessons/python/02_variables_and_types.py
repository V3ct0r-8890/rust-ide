# Lesson 2: Variables and basic types
#
# A variable is a name for a value. You do not declare a type: Python works it out.

age = 30
price = 19.99
name = "Ada"
ready = True
nothing = None  # the "no value" value

print(age, price, name, ready, nothing)

# type() tells you what kind of value something is.
print(type(age), type(price), type(name), type(ready), type(nothing))

# A name can be re-assigned, even to another type.
age = age + 1
age += 1  # shorthand for age = age + 1
print(age)

# Several assignments at once.
x, y = 3, 4
x, y = y, x  # swap without a temporary variable
print(x, y)
a = b = 0  # both names get 0

# Converting between types.
print(int("42") + 1)
print(float("2.5") * 2)
print(str(100) + " percent")
print(int(3.99))  # cuts off the fraction
print(bool(0), bool(""), bool("text"), bool([]))  # empty/zero values are False

# Naming rules: letters, digits and _, not starting with a digit. Style: lower_case_with_underscores.
user_name = "grace"
MAX_USERS = 10  # a constant by convention (Python does not enforce it)
print(user_name, MAX_USERS)

# isinstance checks the type.
print(isinstance(age, int), isinstance(name, int))

# Try it: swap the values of two variables using a third variable instead of the tuple trick.
