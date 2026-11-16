# Lesson 17: Dates and times

from datetime import date, datetime, time, timedelta, timezone

# Creating values. (Fixed dates are used so the output is always the same.)
birthday = date(1990, 6, 15)
meeting = datetime(2026, 3, 14, 9, 30)
print(birthday, meeting)
print(birthday.year, birthday.month, birthday.day, birthday.weekday())  # Monday is 0
print(birthday.strftime("%A, %d %B %Y"))

# "Now" and "today" exist, but their output changes every run.
now = datetime.now()
print(type(now).__name__, now.year >= 2026)

# Arithmetic with timedelta.
one_week = timedelta(weeks=1)
print(meeting + one_week)
print(meeting - timedelta(hours=10, minutes=45))
print(date(2026, 12, 25) - date(2026, 3, 14))             # a timedelta
print((date(2026, 12, 25) - date(2026, 3, 14)).days)

# Compare dates directly.
print(date(2026, 1, 1) < date(2026, 6, 1))

# Text <-> date.
parsed = datetime.strptime("2026-03-14 09:30", "%Y-%m-%d %H:%M")
print(parsed == meeting)
print(meeting.isoformat())
print(datetime.fromisoformat("2026-03-14T09:30:00"))

# Common format codes: %Y year  %m month  %d day  %H hour  %M minute  %S second
#                      %A weekday name  %B month name  %j day of year

# Time zones: attach one to make a datetime "aware".
utc = datetime(2026, 3, 14, 9, 30, tzinfo=timezone.utc)
bangkok = utc.astimezone(timezone(timedelta(hours=7)))
print(utc, "->", bangkok)

# For named zones (like "Asia/Bangkok") use the zoneinfo module (needs tzdata on Windows):
try:
    from zoneinfo import ZoneInfo
    print(utc.astimezone(ZoneInfo("Europe/London")))
except Exception as error:
    print("zoneinfo not available:", type(error).__name__)

# Useful calculations.
def age(born, today):
    return today.year - born.year - ((today.month, today.day) < (born.month, born.day))


print(age(birthday, date(2026, 6, 14)), age(birthday, date(2026, 6, 15)))

import calendar
print(calendar.isleap(2028), calendar.monthrange(2026, 2))  # (weekday of the 1st, days in month)
print(calendar.month(2026, 3))

# Measuring how long code takes.
import time as clock
start = clock.perf_counter()
sum(range(1_000_000))
print("took less than 5 seconds:", clock.perf_counter() - start < 5)

# Try it: print how many days are left until the end of this year.
