# Lesson 4: Strings
#
# A string is text. Strings cannot be changed: every operation gives a NEW string.

text = "  The quick brown fox  "
print(text.strip())          # remove spaces at both ends
print(text.upper(), text.lower(), sep=" | ")
print(text.strip().title())
print(len(text))             # number of characters
print("quick" in text)       # membership test
print(text.replace("quick", "slow"))
print(text.find("brown"), text.find("zebra"))  # position, or -1 when missing
print(text.strip().startswith("The"), text.strip().endswith("x"))
print(text.count("o"))

# Indexing and slicing: positions start at 0; negative positions count from the end.
word = "Python"
print(word[0], word[-1])
print(word[0:2])     # "Py": from 0 up to but not including 2
print(word[2:])      # "thon"
print(word[:3])      # "Pyt"
print(word[::-1])    # reversed
print(word[::2])     # every second character

# Splitting and joining.
csv = "red,green,blue"
parts = csv.split(",")
print(parts)
print(" + ".join(parts))
print("one two  three".split())  # splits on any whitespace

# Building text: f-strings are the nicest way.
name, score = "Ada", 9.5
print(f"{name} scored {score:.1f} points")
print(f"{name!r}")                 # shows quotes: handy for debugging
print(f"{name=}, {score=}")        # prints the names too
print(name + " " + str(score))     # concatenation needs str() for numbers
print("-" * 20)                    # repetition

# Escape sequences and raw strings.
print("Tab:\there\nNew line above")
print("Quote: \"hi\" and 'hi'")
print(r"C:\new\table")             # r"..." keeps the backslashes

# Characters and numbers.
print(ord("A"), chr(97))
print("abc" < "abd", "apple" < "Apple")

# Strings are Unicode, so Thai and emoji work.
thai = "สวัสดี"
print(thai, len(thai))

# Checks.
print("123".isdigit(), "abc".isalpha(), "abc123".isalnum(), "  ".isspace())

# Try it: write a function that tells if a word is a palindrome (reads the same backwards).
