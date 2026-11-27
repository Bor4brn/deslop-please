from runqueue.queue import RunQueue


def admit(queue: RunQueue, task: int) -> bool:
    if queue.full():
        return False
    queue.push(task)
    return True


def next_task(queue: RunQueue) -> int | None:
    return queue.pop()
