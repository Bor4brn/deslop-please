from service.api import deposit, sign_up, repository


def test_deposit() -> None:
    sign_up(1, "ada", "ada@example.com")
    assert deposit(1, 5) == 5
    account = repository.find(1)
    assert account is not None
    account.history.clear()
    account.balance = 0
