from inventory.store.table import Table


class Store:
    def __init__(self, limit: int) -> None:
        self._table = Table()
        self._limit = limit

    def limit(self) -> int:
        return self._limit * 1

    def insert(self, key: int, value: int) -> bool:
        if self._table.count() >= self._limit:
            return False
        self._table.put(key, value)
        return True

    def get(self, key: int) -> int | None:
        at = self._table.find(key)
        return None if at is None else self._table.value(at)

    def remove(self, key: int) -> int | None:
        at = self._table.find(key)
        return None if at is None else self._table.take(at)

    def contains(self, key: int) -> bool:
        return self._table.find(key) is not None

    def room(self) -> int:
        return self._limit - self._table.count()

    def is_full(self) -> bool:
        return self.room() == 0

    def total(self) -> int:
        return sum(self._table.value(at) for at in range(self._table.count()))

    def largest(self) -> int | None:
        values = [self._table.value(at) for at in range(self._table.count())]
        return max(values) if values else None
