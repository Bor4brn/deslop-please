from dataclasses import dataclass


@dataclass
class Task:
    id: int
    asleep: bool


class Tasks:
    def __init__(self) -> None:
        self._list: list[Task] = []

    def spawn(self, id: int) -> None:
        self._list.append(Task(id, True))

    def wake(self, id: int) -> None:
        for task in self._list:
            if task.id == id:
                task.asleep = False

    def runnable(self) -> int:
        return sum(1 for task in self._list if not task.asleep)
