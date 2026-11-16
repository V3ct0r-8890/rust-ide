# Lesson 23: A tour of the standard library
#
# "Batteries included": Python ships with modules for most everyday jobs.

import collections
import itertools
import json
import random
import re
import statistics
import sqlite3
import hashlib
import base64
import uuid
import urllib.parse
import enum

# statistics
data = [2, 4, 4, 4, 5, 5, 7, 9]
print(statistics.mean(data), statistics.median(data), statistics.mode(data), round(statistics.stdev(data), 3))

# collections
queue = collections.deque([1, 2, 3], maxlen=3)
queue.append(4)                          # drops the oldest item
queue.appendleft(0)
print(queue)
Point = collections.namedtuple("Point", "x y")
print(Point(1, 2)._replace(x=10))
chain = collections.ChainMap({"a": 1}, {"a": 2, "b": 3})
print(chain["a"], chain["b"])

# enum: named constants
class Color(enum.Enum):
    RED = 1
    GREEN = 2

print(Color.RED, Color(2), Color.GREEN.name, [c.name for c in Color])

# json
document = json.dumps({"name": "Ada", "tags": ["a", "b"], "ok": True}, indent=2, sort_keys=True)
print(document)
print(json.loads(document)["tags"])

# random (seeded for repeatable output)
random.seed(1)
deck = list(range(1, 11))
random.shuffle(deck)
print(deck, random.sample(deck, 3), round(random.random(), 3))

# hashing, encoding and ids
print(hashlib.sha256(b"hello").hexdigest()[:16])
print(base64.b64encode(b"hello").decode(), base64.b64decode("aGVsbG8=").decode())
print(len(str(uuid.uuid4())))

# urls
url = urllib.parse.urlparse("https://example.com:8080/path/page?x=1&y=two#top")
print(url.hostname, url.port, url.path, urllib.parse.parse_qs(url.query))
print(urllib.parse.quote("a b&c"))

# sqlite3: a complete SQL database in a file or in memory
db = sqlite3.connect(":memory:")
db.execute("create table people (name text, age integer)")
db.executemany("insert into people values (?, ?)", [("Ada", 36), ("Linus", 28), ("Grace", 45)])
for row in db.execute("select name, age from people where age > ? order by age", (30,)):
    print(row)
print(db.execute("select avg(age) from people").fetchone()[0])
db.close()

# itertools / functools / operator
from operator import itemgetter
people = [("Ada", 36), ("Linus", 28)]
print(sorted(people, key=itemgetter(1)))
print(list(itertools.combinations("abc", 2)))

# shutil, glob, tempfile, zipfile
import shutil, tempfile, zipfile, os
folder = tempfile.mkdtemp()
with open(os.path.join(folder, "a.txt"), "w") as f:
    f.write("hello")
archive = shutil.make_archive(os.path.join(folder, "backup"), "zip", folder)
print([name for name in zipfile.ZipFile(archive).namelist()])
shutil.rmtree(folder)

# Where to look next: https://docs.python.org/3/library/   (also: help(module) in the interpreter)
# Try it: use sqlite3 to store your own small address book.
