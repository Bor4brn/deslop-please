from dataclasses import dataclass


@dataclass(frozen=True)
class Limit:
    """The most tasks a queue holds."""

    most: int


class RunQueue:
    def __init__(self, limit: int) -> None:
        self._items: list[int] = []
        self.head = 0
        self._limit = Limit(limit)

    def push(self, task: int) -> bool:
        if self.backlog() >= self._limit.most:
            return False
        self._items.append(task)
        return True

    def pop(self) -> int | None:
        if self.head >= len(self._items):
            return None
        task = self._items[self.head]
        self.head += 1
        return task

    def backlog(self) -> int:
        return len(self._items) - self.head

    def limit(self) -> Limit:
        return self._limit
