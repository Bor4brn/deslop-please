from runqueue.queue import RunQueue


def backlog(queue: RunQueue) -> int:
    return queue.limit - queue.head


def drained(queue: RunQueue) -> bool:
    return queue.head == 0
