class RunQueue:
    def __init__(self, limit: int) -> None:
        self.items: list[int] = []
        self.head = 0
        self.limit = limit

    def full(self) -> bool:
        return len(self.items) - self.head >= self.limit
