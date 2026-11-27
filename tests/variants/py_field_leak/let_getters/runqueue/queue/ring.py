from typing import Callable, TypeVar

R = TypeVar("R")


class RunQueue:
    def __init__(self, limit: int) -> None:
        self._items: list[int] = []
        self._head = 0
        self._limit = limit

    def full(self) -> bool:
        return len(self._items) - self._head >= self._limit

    def items(self) -> list[int]:
        items = self._items
        return items

    def with_items(self, each: Callable[[list[int]], R]) -> R:
        return each(self._items)

    def head(self) -> int:
        head = self._head
        return head

    def set_head(self, head: int) -> None:
        self._head = head
