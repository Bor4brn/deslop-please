from runqueue.queue import RunQueue


def admit(queue: RunQueue, task: int) -> bool:
    return queue.push(task)


def next_task(queue: RunQueue) -> int | None:
    return queue.pop()
