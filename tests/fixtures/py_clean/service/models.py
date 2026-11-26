# Öğrenci hesapları · bakiye “₺” cinsinden.
from dataclasses import dataclass, field


@dataclass(frozen=True)
class User:
    id: int
    name: str
    email: str


@dataclass
class Account:
    owner: User
    balance: int = 0
    history: list[int] = field(default_factory=list)

    def deposit(self, amount: int) -> None:
        if amount <= 0:
            raise ValueError("amount must be positive")
        self.balance += amount
        self.history.append(amount)

    def last(self) -> int | None:
        return self.history[-1] if self.history else None
