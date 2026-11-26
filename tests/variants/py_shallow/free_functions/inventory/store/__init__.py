from inventory.store.table import Table


class Store:
    def __init__(self, limit: int) -> None:
        self.table = Table()
        self.most = limit


def limit(store: Store) -> int:
    return store.most * 1


def insert(store: Store, key: int, value: int) -> bool:
    if store.table.count() >= store.most:
        return False
    store.table.put(key, value)
    return True


def get(store: Store, key: int) -> int | None:
    at = store.table.find(key)
    return None if at is None else store.table.value(at)


def remove(store: Store, key: int) -> int | None:
    at = store.table.find(key)
    return None if at is None else store.table.take(at)


def contains(store: Store, key: int) -> bool:
    return store.table.find(key) is not None


def room(store: Store) -> int:
    return store.most - store.table.count()


def is_full(store: Store) -> bool:
    return room(store) == 0


def total(store: Store) -> int:
    return sum(store.table.value(at) for at in range(store.table.count()))


def largest(store: Store) -> int | None:
    values = [store.table.value(at) for at in range(store.table.count())]
    return max(values) if values else None
