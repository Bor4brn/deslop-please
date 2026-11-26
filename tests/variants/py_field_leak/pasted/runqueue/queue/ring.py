class RunQueue:
    def __init__(self, limit: int) -> None:
        self.items: list[int] = []
        self.head = 0
        self.limit = limit

    def full(self) -> bool:
        return len(self.items) - self.head >= self.limit


def admit(queue: RunQueue, task: int) -> bool:
    if queue.full():
        return False
    queue.items.append(task)
    return True


def next_task(queue: RunQueue) -> int | None:
    if queue.head >= len(queue.items):
        return None
    task = queue.items[queue.head]
    queue.head += 1
    return task


def backlog(queue: RunQueue) -> int:
    return len(queue.items) - queue.head


def drained(queue: RunQueue) -> bool:
    return queue.head == len(queue.items)
