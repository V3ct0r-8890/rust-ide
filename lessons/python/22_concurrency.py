# Lesson 22: Threads, processes and async
#
# Three tools, and when to use them:
#   threading     many things waiting at the same time (network, disk). Python's GIL means threads
#                 do not make CPU-heavy calculations faster.
#   multiprocessing   CPU-heavy work spread over several cores.
#   asyncio       thousands of waiting tasks in one thread.

import asyncio
import threading
import time
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor


def slow_task(name, seconds):
    time.sleep(seconds)                   # pretend to wait for the network
    return f"{name} done after {seconds}s"


def square(n):
    return n * n


# --- threads: a pool runs tasks and collects the results in order ---
start = time.perf_counter()
with ThreadPoolExecutor(max_workers=4) as pool:
    results = list(pool.map(lambda i: slow_task(f"task{i}", 0.3), range(4)))
print(results)
print("4 waits of 0.3s took under a second:", time.perf_counter() - start < 1.0)

# Raw threads and a lock: protect data that several threads change.
counter = 0
lock = threading.Lock()


def add_many():
    global counter
    for _ in range(10_000):
        with lock:
            counter += 1


threads = [threading.Thread(target=add_many) for _ in range(4)]
for t in threads:
    t.start()
for t in threads:
    t.join()
print("counter =", counter)               # always 40000 with the lock


# --- asyncio: tasks give control back with `await` ---
async def fetch(name, seconds):
    await asyncio.sleep(seconds)          # does not block the other tasks
    return f"{name} finished"


async def main():
    started = time.perf_counter()
    results = await asyncio.gather(fetch("a", 0.3), fetch("b", 0.3), fetch("c", 0.3))
    print(results)
    print("ran together:", time.perf_counter() - started < 0.8)

    # Timeouts.
    try:
        await asyncio.wait_for(fetch("slow", 5), timeout=0.1)
    except asyncio.TimeoutError:
        print("gave up waiting")

    # A producer and a consumer talking through a queue.
    queue = asyncio.Queue()

    async def producer():
        for i in range(3):
            await queue.put(i)
        await queue.put(None)

    async def consumer():
        while (item := await queue.get()) is not None:
            print("got", item)

    await asyncio.gather(producer(), consumer())


asyncio.run(main())


# --- processes: need the __main__ guard because child processes import this file ---
if __name__ == "__main__":
    with ProcessPoolExecutor(max_workers=2) as pool:
        print(list(pool.map(square, range(6))))

# Try it: make slow_task fail with an exception for one input and see how pool.map reports it.
