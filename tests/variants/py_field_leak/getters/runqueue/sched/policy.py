from runqueue.queue import RunQueue


def admit(queue: RunQueue, task: int) -> bool:
    if queue.full():
        return False
    queue.items.append(task)
    return True


def next_task(queue: RunQueue) -> int | None:
    if queue.head() >= len(queue.items):
        return None
    task = queue.items[queue.head()]
    queue.set_head(queue.head() + 1)
    return task
