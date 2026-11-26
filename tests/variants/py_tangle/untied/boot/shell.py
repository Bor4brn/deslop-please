from boot.kernel import tick
from boot.kernel.tasks import Tasks
from boot.kernel.timers import Timers


def boot(ids: list[int], until: int) -> int:
    tasks = Tasks()
    timers = Timers()
    for id in ids:
        tasks.spawn(id)
        timers.arm(id, 10)
    awake = 0
    for now in range(until):
        awake = tick(now, tasks, timers)
    return awake
