# Lesson 13: Input and files
#
# Press F5. When the program asks a question, type the answer into the input line of the
# Output window and press Enter (or just press Enter to accept the default).

import os
import tempfile
from pathlib import Path

try:
    name = input("What is your name? ").strip() or "friend"
except EOFError:        # no keyboard attached
    name = "friend"
print(f"Hello, {name}!")

# input() always gives text: convert it yourself.
# age = int(input("Age? "))

# Work inside a temporary folder so we never touch your real files.
folder = Path(tempfile.mkdtemp(prefix="lesson13-"))
file = folder / "notes.txt"          # the / operator joins path parts

# Write. `with` closes the file for you, even if an error happens.
with open(file, "w", encoding="utf-8") as handle:
    handle.write("first line\n")
    handle.write("second line\n")

# Append.
with open(file, "a", encoding="utf-8") as handle:
    print(f"third line for {name}", file=handle)

# Read everything.
text = file.read_text(encoding="utf-8")
print(f"--- {file.name} ---\n{text}---")

# Read line by line (good for big files).
with open(file, encoding="utf-8") as handle:
    for number, line in enumerate(handle, start=1):
        print(f"{number:>2}: {line.rstrip()}")

# pathlib: information and folders.
print(file.exists(), file.is_file(), file.suffix, file.stem, file.stat().st_size)
(folder / "other.txt").write_text("x")
print(sorted(p.name for p in folder.iterdir()))
print(sorted(p.name for p in folder.glob("*.txt")))

# Text files with structured data.
import csv
import json

people = [{"name": "Ada", "age": 36}, {"name": "Linus", "age": 28}]
(folder / "people.json").write_text(json.dumps(people, indent=2))
print(json.loads((folder / "people.json").read_text()))

with open(folder / "people.csv", "w", newline="", encoding="utf-8") as handle:
    writer = csv.DictWriter(handle, fieldnames=["name", "age"])
    writer.writeheader()
    writer.writerows(people)
with open(folder / "people.csv", newline="", encoding="utf-8") as handle:
    for row in csv.DictReader(handle):
        print(row["name"], row["age"])

# Always expect missing files.
try:
    (folder / "missing.txt").read_text()
except FileNotFoundError:
    print("missing.txt does not exist")

# Clean up.
import shutil
shutil.rmtree(folder)
print("cleaned up:", not folder.exists())

# Try it: write a program that counts the lines and words of a text file.
