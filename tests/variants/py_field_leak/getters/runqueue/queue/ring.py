class RunQueue:
    def __init__(self, limit: int) -> None:
        self._items: list[int] = []
        self._head = 0
        self._limit = limit

    @property
    def items(self) -> list[int]:
        return self._items

    def head(self) -> int:
        return self._head

    def set_head(self, head: int) -> None:
        self._head = head

    def full(self) -> bool:
        return len(self._items) - self._head >= self._limit
