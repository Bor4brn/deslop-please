class RunQueue:
    def __init__(self, limit: int) -> None:
        self._items: list[int] = []
        self._head = 0
        self._limit = limit

    def push(self, task: int) -> bool:
        if self.backlog() >= self._limit:
            return False
        self._items.append(task)
        return True

    def pop(self) -> int | None:
        if self._head >= len(self._items):
            return None
        task = self._items[self._head]
        self._head += 1
        return task

    def backlog(self) -> int:
        return len(self._items) - self._head
