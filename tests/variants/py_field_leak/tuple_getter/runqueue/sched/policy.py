from runqueue.queue import RunQueue


def admit(queue: RunQueue, task: int) -> bool:
    if queue.full():
        return False
    items, _ = queue.parts()
    items.append(task)
    return True


def next_task(queue: RunQueue) -> int | None:
    items, head = queue.parts()
    if head >= len(items):
        return None
    queue.advance()
    return items[head]
