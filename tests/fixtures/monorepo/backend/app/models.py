from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from app.db import Session


@dataclass
class User:
    id: int
    name: str

    def save(self, session: Session) -> None:
        session.add(self)
