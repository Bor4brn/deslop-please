from inventory.store.table import Table


class Store:
    """Hands every call on to its table."""

    def __init__(self, limit: int) -> None:
        self._table = Table()

    def limit(self) -> int:
        return self._table.limit()

    def insert(self, key: int, value: int) -> bool:
        return self._table.insert(key, value)

    def get(self, key: int) -> int | None:
        return self._table.get(key)

    def remove(self, key: int) -> int | None:
        return self._table.remove(key)

    def contains(self, key: int) -> bool:
        return self._table.contains(key)

    def room(self) -> int:
        return self._table.room()

    def is_full(self) -> bool:
        return self._table.is_full()

    def total(self) -> int:
        return self._table.total()

    def largest(self) -> int | None:
        return self._table.largest()
