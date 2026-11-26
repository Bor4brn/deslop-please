class RunQueue:
    def __init__(self, limit: int) -> None:
        self._items: list[int] = []
        self._head = 0
        self._limit = limit

    def full(self) -> bool:
        return len(self._items) - self._head >= self._limit

    def parts(self) -> tuple[list[int], int]:
        return (self._items, self._head)

    def advance(self) -> None:
        self._head += 1
