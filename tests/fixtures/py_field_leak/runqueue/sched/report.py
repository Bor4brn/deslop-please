from runqueue.queue import RunQueue


def backlog(queue: RunQueue) -> int:
    return len(queue.items) - queue.head


def drained(queue: RunQueue) -> bool:
    return queue.head == len(queue.items)
