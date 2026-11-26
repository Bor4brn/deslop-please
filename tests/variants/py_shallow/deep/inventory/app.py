from inventory.store import Store


def run(pairs: list[tuple[int, int]]) -> int:
    store = Store(8)
    store.load(pairs)
    return store.summary([key for key, _ in pairs])
