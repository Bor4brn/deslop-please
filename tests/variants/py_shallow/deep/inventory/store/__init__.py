from inventory.store.table import Table


class Store:
    def __init__(self, limit: int) -> None:
        self._table = Table()
        self._limit = limit

    def load(self, pairs: list[tuple[int, int]]) -> None:
        for key, value in pairs:
            if self._is_full() or not self._insert(key, value):
                self._remove(key)

    def summary(self, keys: list[int]) -> int:
        kept = sum(1 for key in keys if self._contains(key))
        found = sum(1 for key in keys if self._get(key) is not None)
        rest = kept + found + self._room() + self._limit
        return self._total() + (self._largest() or 0) + rest

    def _insert(self, key: int, value: int) -> bool:
        if self._table.count() >= self._limit:
            return False
        self._table.put(key, value)
        return True

    def _get(self, key: int) -> int | None:
        at = self._table.find(key)
        return None if at is None else self._table.value(at)

    def _remove(self, key: int) -> int | None:
        at = self._table.find(key)
        return None if at is None else self._table.take(at)

    def _contains(self, key: int) -> bool:
        return self._table.find(key) is not None

    def _room(self) -> int:
        return self._limit - self._table.count()

    def _is_full(self) -> bool:
        return self._room() == 0

    def _total(self) -> int:
        return sum(self._table.value(at) for at in range(self._table.count()))

    def _largest(self) -> int | None:
        values = [self._table.value(at) for at in range(self._table.count())]
        return max(values) if values else None
