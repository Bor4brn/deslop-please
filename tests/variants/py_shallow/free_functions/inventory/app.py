from inventory import store as inventory
from inventory.store import Store


def run(pairs: list[tuple[int, int]]) -> int:
    store = Store(8)
    for key, value in pairs:
        if inventory.is_full(store) or not inventory.insert(store, key, value):
            inventory.remove(store, key)
    kept = sum(1 for key, _ in pairs if inventory.contains(store, key))
    found = sum(1 for key, _ in pairs if inventory.get(store, key) is not None)
    rest = kept + found + inventory.room(store) + inventory.limit(store)
    return inventory.total(store) + (inventory.largest(store) or 0) + rest
