from inventory.store import Store


def run(pairs: list[tuple[int, int]]) -> int:
    store = Store(8)
    for key, value in pairs:
        if store.is_full() or not store.insert(key, value):
            store.remove(key)
    kept = sum(1 for key, _ in pairs if store.contains(key))
    found = sum(1 for key, _ in pairs if store.get(key) is not None)
    rest = kept + found + store.room() + store.limit()
    return store.total() + (store.largest() or 0) + rest
