from fastapi import APIRouter

from app.db import Session
from app.models import User

router = APIRouter()
session = Session()


@router.post("/users/{id}")
def create(id: int, name: str) -> dict[str, object]:
    user = User(id=id, name=name)
    user.save(session)
    return {"id": user.id, "name": user.name}
