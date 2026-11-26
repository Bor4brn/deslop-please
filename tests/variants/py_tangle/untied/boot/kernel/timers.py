class Timers:
    def __init__(self) -> None:
        self._due: list[tuple[int, int]] = []

    def arm(self, id: int, at: int) -> None:
        self._due.append((id, at))

    def fire(self, now: int) -> list[int]:
        fired = [id for id, at in self._due if at <= now]
        self._due = [(id, at) for id, at in self._due if at > now]
        return fired
