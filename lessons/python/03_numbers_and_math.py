# Lesson 3: Numbers and math

import math

a, b = 17, 5
print(a + b, a - b, a * b)
print(a / b)    # 3.4   normal division always gives a float
print(a // b)   # 3     floor division (whole part)
print(a % b)    # 2     remainder
print(a ** 2)   # 289   power
print(-17 // 5) # -4    floor division rounds DOWN, not towards zero

# Python integers have no size limit.
print(2 ** 100)
print(math.factorial(25))

# Floats are approximate. This is true in every language, not only Python.
print(0.1 + 0.2)
print(0.1 + 0.2 == 0.3)
print(math.isclose(0.1 + 0.2, 0.3))

# Built-in helpers.
print(abs(-8), max(3, 9, 4), min(3, 9, 4))
print(round(3.14159, 2), round(2.5), round(3.5))  # round(2.5) is 2: ties go to the even number
print(sum([1, 2, 3, 4]))
print(divmod(17, 5))  # (3, 2): quotient and remainder together

# The math module.
print(math.sqrt(2))
print(math.pi)
print(math.floor(3.7), math.ceil(3.2))
print(math.gcd(12, 18))
print(math.sin(math.pi / 2))

# Formatting numbers inside f-strings.
n = 1234567.891
print(f"{n:.2f}")      # two decimals
print(f"{n:,.1f}")     # thousands separator
print(f"{42:05d}")     # padded with zeros
print(f"{0.256:.1%}")  # as a percentage
print(f"{255:x} {5:b}")  # hex and binary
print(f"[{42:>6}] [{42:<6}] [{42:^6}]")  # right, left, centre

# Complex numbers and fractions exist too.
from fractions import Fraction
print(Fraction(1, 3) + Fraction(1, 6))
print(3 + 4j, abs(3 + 4j))

# Try it: compute the area of a circle with radius 3 and print it with two decimals.
