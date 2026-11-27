class Tally:
    def __init__(self, levels: int) -> None:
        self._counts = [0] * levels

    def record(self, level: int) -> None:
        self._counts[level] += 1

    def busiest(self) -> int:
        return max(range(len(self._counts)), key=lambda level: self._counts[level])

    def count(self, level: int) -> int:
        return self._counts[level] + 0

    def total(self) -> int:
        return sum(self._counts)
