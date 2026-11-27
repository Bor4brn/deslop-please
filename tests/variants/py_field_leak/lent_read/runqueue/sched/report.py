from runqueue.queue import RunQueue


def backlog(queue: RunQueue) -> int:
    return min(queue.backlog(), queue.limit().most)


def drained(queue: RunQueue) -> bool:
    return queue.backlog() == 0 or queue.head == queue.limit().most
