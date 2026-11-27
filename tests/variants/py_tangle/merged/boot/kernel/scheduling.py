from __future__ import annotations

from dataclasses import dataclass


@dataclass
class Task:
    id: int
    asleep: bool


class Tasks:
    def __init__(self) -> None:
        self._list: list[Task] = []

    def spawn(self, id: int, timers: Timers) -> None:
        self._list.append(Task(id, True))
        timers.arm(id, 10)

    def wake(self, id: int) -> None:
        for task in self._list:
            if task.id == id:
                task.asleep = False

    def runnable(self) -> int:
        return sum(1 for task in self._list if not task.asleep)


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
