# Lesson 21: Command-line arguments and environment variables

import argparse
import os
import platform
import sys

# sys.argv holds the arguments: the first item is this script's path.
print("raw arguments:", sys.argv[1:])

# argparse builds a real command-line interface: help text, types, defaults and error messages.
parser = argparse.ArgumentParser(description="Greets people.")
parser.add_argument("names", nargs="*", default=["world"], help="who to greet")
parser.add_argument("-n", "--count", type=int, default=1, help="how many times")
parser.add_argument("-v", "--verbose", action="store_true", help="say more")
parser.add_argument("--shout", action="store_true")

# parse_args() reads sys.argv. We also try fixed examples so the lesson works without arguments.
examples = [["-v", "-n", "2", "Ada", "Linus"], ["--shout"], []]
for example in examples:
    options = parser.parse_args(example)
    print(example, "->", options)

options = parser.parse_args()   # the real arguments (none when run with F5)
for _ in range(options.count):
    for name in options.names:
        text = f"hello {name}"
        print(text.upper() if options.shout else text)

# A bad argument makes argparse print a message and exit with code 2.
try:
    parser.parse_args(["--count", "many"])
except SystemExit as exit_:
    print("argparse exited with", exit_.code)

# Environment variables.
print(os.environ.get("HOME") or os.environ.get("USERPROFILE"))
print(os.environ.get("LESSON_LEVEL", "default"))
os.environ["MY_SETTING"] = "on"            # only for this process and its children
print(os.getenv("MY_SETTING"))

# Information about the machine.
print(platform.system(), platform.machine(), sys.version.split()[0])
print(os.getcwd())
print(os.cpu_count())

# Running other programs.
import subprocess
result = subprocess.run([sys.executable, "-c", "print('from a child process')"], capture_output=True, text=True)
print(result.stdout.strip(), "| exit code", result.returncode)

# Exit codes: 0 means success; anything else means failure.
# sys.exit(1) would stop the program right here with code 1.
if __name__ == "__main__":
    print("done")
    sys.exit(0)

# Try it: add a --repeat option and a required argument, then look at parser.print_help().
