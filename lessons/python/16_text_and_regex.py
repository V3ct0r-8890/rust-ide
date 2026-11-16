# Lesson 16: Working with text: regular expressions and formatting

import re

text = "Contact: ada@example.com, linus@kernel.org or call 555-1234 on 2026-03-14."

# Regular expressions describe patterns of text. Use raw strings r"..." for them.
print(re.search(r"\d+", text).group())          # first run of digits
print(re.findall(r"\d{4}-\d{2}-\d{2}", text))   # every date
print(re.findall(r"[\w.]+@[\w.]+\w", text))     # simple e-mail pattern
print(re.sub(r"\d", "#", "call 555-1234"))      # replace
print(bool(re.fullmatch(r"\d{3}-\d{4}", "555-1234")))
print(re.split(r"[,;]\s*", "a, b;c,  d"))

# Groups pull out parts of a match; names make the code readable.
pattern = re.compile(r"(?P<year>\d{4})-(?P<month>\d{2})-(?P<day>\d{2})")
match = pattern.search(text)
if match:
    print(match.group("year"), match.group("month"), match.group("day"))
    print(match.groupdict())
    print(match.start(), match.end())

# Replace with a function.
print(re.sub(r"\d+", lambda m: str(int(m.group()) * 2), "3 apples and 10 pears"))

# Quick cheat sheet:
#  .  any character      \d digit     \w letter/digit/_     \s space
#  *  zero or more       +  one or more      ?  optional    {2,4} two to four
#  ^  start of text      $  end of text      [abc] one of   (a|b) a or b
print(re.findall(r"\b[A-Z]\w*", "Alice met Bob in Paris"))

# Formatting tables with f-strings.
rows = [("apple", 3, 0.5), ("watermelon", 12, 2.25), ("fig", 100, 0.125)]
print(f"{'item':<12}{'qty':>5}{'price':>9}")
for item, qty, price in rows:
    print(f"{item:<12}{qty:>5}{price:>9.2f}")

# Other useful text tools.
import string, textwrap
print(string.ascii_lowercase[:5], string.digits)
long = "Python is a programming language that lets you work quickly and integrate systems well."
print(textwrap.fill(long, width=30))
print(textwrap.shorten(long, width=30, placeholder="..."))
print("hello world".capitalize(), "snake_case_name".replace("_", " ").title())
print("a\tb".expandtabs(4), "x".center(5, "*"), "7".zfill(3))

# Word frequency in a few lines.
from collections import Counter
words = re.findall(r"[a-z']+", "It was the best of times, it was the worst of times".lower())
print(Counter(words).most_common(3))

# Try it: check whether a password has at least 8 characters, a digit and an upper-case letter.
