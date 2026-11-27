from service.models import User
from service.storage import Repository

repository = Repository()


def sign_up(id: int, name: str, email: str) -> str:
    account = repository.open(User(id=id, name=name, email=email))
    return f"{account.owner.name} <{account.owner.email}>"


def deposit(user_id: int, amount: int) -> int:
    account = repository.find(user_id)
    if account is None:
        raise KeyError(user_id)
    account.deposit(amount)
    return account.balance
