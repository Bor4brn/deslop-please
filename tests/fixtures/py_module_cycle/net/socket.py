from net.kinds import Stream

BACKLOG_MOST = 16


def listen(backlog: int) -> "Stream | None":
    return Stream.open(backlog)
