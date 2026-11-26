from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from boot.kernel.timers import Timers


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
