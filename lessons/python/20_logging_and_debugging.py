# Lesson 20: Debugging and logging
#
# 1. USE THE IDE'S DEBUGGER. Put the cursor on the line `total += price` below and press Ctrl+F8
#    to set a breakpoint, then start debugging with Shift+F5 (Debug menu). You can then step through the code
#    line by line, look at the variables, and see how the values change.
#
# 2. print() is the quickest tool: print the values you doubt.
# 3. logging is print() for grown-ups: levels, timestamps and the ability to switch it off.

import logging
import sys

logging.basicConfig(
    stream=sys.stdout,
    level=logging.DEBUG,                          # change to INFO or WARNING to see less
    format="%(levelname)-8s %(funcName)s: %(message)s",
)
log = logging.getLogger("lesson20")


def total_price(prices, discount):
    log.debug("prices=%s discount=%s", prices, discount)   # %s style: only formatted if shown
    total = 0
    for price in prices:
        total += price                                     # <- put a breakpoint here
    total *= 1 - discount
    log.info("total after discount: %.2f", total)
    return total


total_price([10, 20, 30], 0.1)
log.warning("this is a warning")
log.error("this is an error")

# Logging an exception with its traceback.
try:
    1 / 0
except ZeroDivisionError:
    log.exception("calculation failed")     # includes the traceback

# Reading a traceback: read from the BOTTOM. The last line says what went wrong; the lines above
# show the path of function calls that led to it. Let us make one and show it.
import traceback


def level_three():
    return int("not a number")


def level_two():
    return level_three()


def level_one():
    return level_two()


try:
    level_one()
except ValueError:
    print(traceback.format_exc())

# A buggy function and its fix.
def average(numbers):
    return sum(numbers) / len(numbers)           # crashes for an empty list


def safe_average(numbers):
    if not numbers:
        return 0.0
    return sum(numbers) / len(numbers)


print(safe_average([]), safe_average([2, 4]))

# Debugging helpers.
value = {"a": [1, 2, 3], "b": {"c": 4}}
import pprint
pprint.pprint(value)
print(type(value), len(value), dir(value)[-3:])
print(f"{value=}")

# breakpoint() starts the Python debugger in a terminal; the IDE debugger is easier.
# Try it: set a breakpoint, run in debug mode and watch `total` grow in the variables view.
