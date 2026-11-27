class Window:
    def __init__(self, size: int) -> None:
        self._size = size
        self._latencies: list[int] = []

    def push(self, latency: int) -> None:
        self._latencies = (self._latencies + [latency])[-self._size :]

    def mean(self) -> int:
        return sum(self._latencies) // max(len(self._latencies), 1)

    def worst(self) -> int:
        return max(self._latencies, default=0)
