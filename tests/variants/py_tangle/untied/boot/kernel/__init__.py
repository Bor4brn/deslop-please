from boot.kernel.tasks import Tasks
from boot.kernel.timers import Timers


def tick(now: int, tasks: Tasks, timers: Timers) -> int:
    for id in timers.fire(now):
        tasks.wake(id)
    return tasks.runnable()
