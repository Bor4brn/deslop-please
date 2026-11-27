class RunQueue:
    def __init__(self, limit: int) -> None:
        self._slots = _Slots()
        self._limit = limit

    def full(self) -> bool:
        return self._slots.waiting() >= self._limit

    def push(self, task: int) -> None:
        self._slots.push(task)

    def pop(self) -> int | None:
        return self._slots.pop()

    def waiting(self) -> int:
        return self._slots.waiting()


class _Slots:
    def __init__(self) -> None:
        self._items: list[int] = []
        self._head = 0

    def push(self, task: int) -> None:
        self._items.append(task)

    def pop(self) -> int | None:
        if self._head >= len(self._items):
            return None
        task = self._items[self._head]
        self._head += 1
        return task

    def waiting(self) -> int:
        return len(self._items) - self._head
