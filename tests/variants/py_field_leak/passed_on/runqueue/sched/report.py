from runqueue.queue import RunQueue


def backlog(queue: RunQueue) -> int:
    return queue.waiting()


def drained(queue: RunQueue) -> bool:
    return queue.waiting() == 0
