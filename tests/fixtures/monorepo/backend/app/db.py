from app.models import User


class Session:
    def __init__(self) -> None:
        self._users: dict[int, User] = {}

    def add(self, user: User) -> None:
        self._users[user.id] = user

    def get(self, id: int) -> User | None:
        return self._users.get(id)
