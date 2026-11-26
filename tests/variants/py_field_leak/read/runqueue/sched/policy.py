from runqueue.queue import RunQueue


def admit(queue: RunQueue, task: int) -> bool:
    return not queue.full() and task >= queue.head


def next_task(queue: RunQueue) -> int | None:
    return queue.head if queue.limit > 0 else None
