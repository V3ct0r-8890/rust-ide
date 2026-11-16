# Lesson 12: Errors and exceptions
#
# When something goes wrong Python raises an exception. If nothing catches it, the program stops
# and prints a traceback. try/except lets you handle the problem instead.

def divide(a, b):
    return a / b


try:
    print(divide(10, 2))
    print(divide(1, 0))
    print("never reached")
except ZeroDivisionError:
    print("cannot divide by zero")

# Several except blocks, and getting the error object.
for text in ["42", "forty", None]:
    try:
        number = int(text)
    except ValueError as error:
        print(f"{text!r}: not a number ({error})")
    except TypeError as error:
        print(f"{text!r}: wrong type ({error})")
    else:
        print(f"{text!r}: ok, {number}")   # runs only when nothing was raised
    finally:
        print("  (finally always runs)")  # cleanup goes here


# Raise your own errors.
def set_age(age):
    if not isinstance(age, int):
        raise TypeError("age must be a whole number")
    if age < 0:
        raise ValueError(f"age cannot be negative: {age}")
    return age


for value in [5, -1, "x"]:
    try:
        print(set_age(value))
    except (ValueError, TypeError) as error:
        print("rejected:", error)


# Your own exception classes.
class InsufficientFunds(Exception):
    def __init__(self, balance, wanted):
        super().__init__(f"balance {balance} is less than {wanted}")
        self.balance = balance
        self.wanted = wanted


def withdraw(balance, amount):
    if amount > balance:
        raise InsufficientFunds(balance, amount)
    return balance - amount


try:
    withdraw(10, 50)
except InsufficientFunds as error:
    print("error:", error, "| missing", error.wanted - error.balance)


# Re-raise with context: "raise ... from ...".
def load_port(text):
    try:
        return int(text)
    except ValueError as error:
        raise RuntimeError(f"bad port setting {text!r}") from error


try:
    load_port("eighty")
except RuntimeError as error:
    print(error, "| caused by:", repr(error.__cause__))

# Missing files are a common error.
try:
    open("this-file-does-not-exist.txt")
except FileNotFoundError as error:
    print("no file:", error.filename)

# assert checks something that must be true while developing.
total = 5
assert total > 0, "total must be positive"

# Style: catch the specific exception you expect. A bare `except:` hides bugs.
# Ask forgiveness (try) rather than permission (many ifs) when the check is hard to do first.

# Try it: write a function that asks for a number with input() and repeats until it gets a valid one.
