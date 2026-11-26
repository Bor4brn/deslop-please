class Rate:
    def __init__(self, first: int) -> None:
        self._first = first
        self._last = first
        self._lines = 0

    def observe(self, second: int) -> None:
        self._last = max(self._last, second)
        self._lines += 1

    def per_second(self) -> int:
        return self._lines // self._span()

    def _span(self) -> int:
        return self._last - self._first + 1
