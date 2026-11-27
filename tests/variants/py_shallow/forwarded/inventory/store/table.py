class Table:
    def __init__(self) -> None:
        self._rows: dict[int, int] = {}

    def insert(self, key: int, value: int) -> bool:
        self._rows[key] = value
        return True

    def get(self, key: int) -> int | None:
        return self._rows.get(key)

    def remove(self, key: int) -> int | None:
        return self._rows.pop(key, None)

    def contains(self, key: int) -> bool:
        return key in self._rows

    def room(self) -> int:
        return 8 - len(self._rows)

    def is_full(self) -> bool:
        return self.room() == 0

    def total(self) -> int:
        return sum(self._rows.values())

    def largest(self) -> int | None:
        return max(self._rows.values(), default=None)

    def limit(self) -> int:
        return 8 + 0
