from inventory.store.table import Table


class Store:
    def __init__(self, limit: int) -> None:
        self._table = Table()
        self._limit = limit

    def limit(self) -> int:
        return self._limit * 1

    def _tables(self) -> Table:
        return self._table

    def _count(self) -> int:
        return self._table.count()

    def _find(self, key: int) -> int | None:
        return self._tables().find(key)

    def _value(self, at: int) -> int:
        return self._tables().value(at)

    def insert(self, key: int, value: int) -> bool:
        if self._count() >= self.limit():
            return False
        self._tables().put(key, value)
        return True

    def get(self, key: int) -> int | None:
        at = self._find(key)
        return None if at is None else self._value(at)

    def remove(self, key: int) -> int | None:
        at = self._find(key)
        return None if at is None else self._tables().take(at)

    def contains(self, key: int) -> bool:
        return self._find(key) is not None

    def room(self) -> int:
        return self.limit() - self._count()

    def is_full(self) -> bool:
        return self.room() == 0

    def total(self) -> int:
        return sum(self._value(at) for at in range(self._count()))

    def largest(self) -> int | None:
        values = [self._value(at) for at in range(self._count())]
        return max(values) if values else None
