# Lesson 14: Modules and packages
#
# A module is a .py file. `import` loads it. The standard library has hundreds of them.

import math
import random
import datetime
import sys
from collections import namedtuple
from pathlib import Path as P   # import one name, and give it another name

print(math.sqrt(16))
random.seed(7)                   # a fixed seed makes "random" numbers repeatable
print(random.randint(1, 6), random.choice(["red", "green"]))
print(datetime.date(2026, 1, 31) + datetime.timedelta(days=1))
print(datetime.datetime(2026, 5, 17, 14, 30).strftime("%A %d %B %Y, %H:%M"))
print(sys.version_info.major, sys.platform)
print(P("a") / "b" / "c.txt")

Point = namedtuple("Point", "x y")
print(Point(1, 2).x)

# Exploring a module.
print(math.__name__)
print([name for name in dir(math) if name.startswith("co")])
help_text = (math.hypot.__doc__ or "").splitlines()[0]
print(help_text)

# Your own module: put this code in a file called shapes.py next to your program:
#
#     # shapes.py
#     def area(width, height):
#         return width * height
#
# and then in another file:
#
#     import shapes
#     print(shapes.area(2, 3))
#     from shapes import area
#
# A folder with an __init__.py file is a package: import mypackage.shapes

# The `if __name__ == "__main__":` pattern: the code below runs when you start this file
# directly (F5), but not when another file imports it. Always use it for the "main program".
def main():
    print("running as the main program")
    print("module name:", __name__)


if __name__ == "__main__":
    main()

# Installing third-party packages: use pip inside a virtual environment, for example
#     python -m venv .venv
#     .venv\Scripts\python -m pip install requests     (Windows)
#     .venv/bin/python -m pip install requests         (Linux and macOS)
# The IDE offers to set up a .venv for a project.

# Try it: create shapes.py with two functions in your project folder and import it here.
