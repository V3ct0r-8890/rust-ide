# Lesson 1: Hello, world
#
# Press F5 to run this file. The output appears in the Output window.
# Python runs the file from the top to the bottom, one line after another.
# Lines starting with # are comments: Python ignores them.

print("Hello, world!")

# print can show several values; it puts a space between them.
print("My name is", "Ada", "and I am learning", "Python")

# f-strings put values inside text: write f before the quote and use {} for the values.
language = "Python"
print(f"{language} is easy to read.")

# print adds a new line at the end. You can change that with end=.
print("No new line here, ", end="")
print("but now there is.")

# A string can have several lines when you use triple quotes.
print("""This text
spans three
lines.""")

# Try it: change the text and press F5 again.
# Try it: remove a closing quote and press Alt+F9 to see how an error message looks.
