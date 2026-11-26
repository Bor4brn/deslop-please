from net.kinds import Stream


def listen(backlog: int) -> "Stream | None":
    return Stream.open(backlog)
