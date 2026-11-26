from net.limits import backlog_most


class Stream:
    def __init__(self, backlog: int) -> None:
        self.backlog = backlog
        self.pending = 0

    @staticmethod
    def open(backlog: int) -> "Stream | None":
        if backlog > backlog_most():
            return None
        return Stream(backlog)

    def accept(self) -> bool:
        room = self.pending < self.backlog
        if room:
            self.pending += 1
        return room
