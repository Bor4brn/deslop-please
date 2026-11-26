from runqueue.queue import RunQueue


def backlog(queue: RunQueue) -> int:
    return queue.backlog()


def drained(queue: RunQueue) -> bool:
    return queue.backlog() == 0
