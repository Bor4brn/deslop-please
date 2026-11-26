from runqueue.queue import RunQueue


def backlog(queue: RunQueue) -> int:
    items, head = queue.parts()
    return len(items) - head


def drained(queue: RunQueue) -> bool:
    items, head = queue.parts()
    return head == len(items)
