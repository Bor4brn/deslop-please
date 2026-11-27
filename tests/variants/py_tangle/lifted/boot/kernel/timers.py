from __future__ import annotations

from boot.kernel import Tasks


class Timers:
    def __init__(self) -> None:
        self._due: list[tuple[int, int]] = []

    def arm(self, id: int, at: int) -> None:
        self._due.append((id, at))

    def fire(self, now: int, tasks: Tasks) -> None:
        for id, at in self._due:
            if at <= now:
                tasks.wake(id)
        self._due = [(id, at) for id, at in self._due if at > now]
