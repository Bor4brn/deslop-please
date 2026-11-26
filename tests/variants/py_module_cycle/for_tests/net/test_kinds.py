from net.kinds import Stream
from net.socket import listen


def stream() -> Stream:
    opened = listen(1)
    assert opened is not None
    return opened


def test_accepts_up_to_the_backlog() -> None:
    assert stream().accept()
