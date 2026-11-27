from service.models import Account, User


class Repository:
    """Accounts by their owner's id, kept in memory."""

    def __init__(self) -> None:
        self._accounts: dict[int, Account] = {}

    def open(self, user: User) -> Account:
        account = Account(owner=user)
        self._accounts[user.id] = account
        return account

    def find(self, user_id: int) -> Account | None:
        return self._accounts.get(user_id)
