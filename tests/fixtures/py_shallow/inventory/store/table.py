class Table:
    def __init__(self) -> None:
        self._keys: list[int] = []
        self._values: list[int] = []

    def count(self) -> int:
        return len(self._keys)

    def put(self, key: int, value: int) -> None:
        self._keys.append(key)
        self._values.append(value)

    def find(self, key: int) -> int | None:
        return self._keys.index(key) if key in self._keys else None

    def value(self, at: int) -> int:
        return self._values[at]

    def take(self, at: int) -> int:
        self._keys.pop(at)
        return self._values.pop(at)
