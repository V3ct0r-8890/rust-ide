# Lesson 19: Testing
#
# Tests are small programs that check your code. Python includes `unittest`, and the very
# popular `pytest` can be installed with pip. Here we use unittest so it works everywhere.
# Press F5: the tests run and the report appears in the Output window.

import unittest


def is_even(n):
    return n % 2 == 0


def clamp(value, low, high):
    return max(low, min(value, high))


def word_count(text):
    return len(text.split())


def divide(a, b):
    if b == 0:
        raise ValueError("division by zero")
    return a / b


class MathTests(unittest.TestCase):
    def test_even_numbers(self):
        self.assertTrue(is_even(2))
        self.assertFalse(is_even(3))
        self.assertTrue(is_even(-4))

    def test_clamp_keeps_value_inside(self):
        self.assertEqual(clamp(5, 0, 10), 5)
        self.assertEqual(clamp(-1, 0, 10), 0)
        self.assertEqual(clamp(11, 0, 10), 10)

    def test_divide(self):
        self.assertAlmostEqual(divide(1, 3), 0.3333, places=3)  # floats: compare loosely

    def test_divide_by_zero_raises(self):
        with self.assertRaises(ValueError):
            divide(1, 0)


class TextTests(unittest.TestCase):
    def setUp(self):                       # runs before EACH test
        self.sentence = "the quick  brown fox"

    def test_counting_words_ignores_extra_spaces(self):
        self.assertEqual(word_count(self.sentence), 4)

    def test_empty_text_has_no_words(self):
        self.assertEqual(word_count(""), 0)

    def test_many_cases_with_subtests(self):
        for text, expected in [("a", 1), ("a b", 2), ("  a   b  c ", 3)]:
            with self.subTest(text=text):
                self.assertEqual(word_count(text), expected)


if __name__ == "__main__":
    unittest.main(verbosity=2)

# Tips:
#  - Name tests like sentences; test one behavior in each.
#  - Write the test first, watch it fail, then make it pass (test-driven development).
#  - With pytest the same tests are plain functions using `assert`:
#        def test_clamp(): assert clamp(11, 0, 10) == 10
#    Install it with: python -m pip install pytest   and run:  python -m pytest
#  - Try it: add a test for a new function `double(n)`; first make it fail, then pass.
