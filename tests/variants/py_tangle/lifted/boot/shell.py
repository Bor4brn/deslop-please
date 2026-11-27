from boot.kernel import Tasks
from boot.kernel.timers import Timers


def boot(ids: list[int], until: int) -> int:
    tasks = Tasks()
    timers = Timers()
    for id in ids:
        tasks.spawn(id, timers)
    awake = 0
    for now in range(until):
        timers.fire(now, tasks)
        awake = tasks.runnable()
    return awake
