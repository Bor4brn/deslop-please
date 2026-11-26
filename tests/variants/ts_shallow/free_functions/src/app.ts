import * as inventory from "./store";
import { Store } from "./store";

export function run(pairs: Array<[number, number]>): number {
  const store = new Store(8);
  for (const [key, value] of pairs) {
    if (inventory.isFull(store) || !inventory.insert(store, key, value)) {
      inventory.remove(store, key);
    }
  }
  const kept = pairs.filter(([key]) => inventory.contains(store, key)).length;
  const found = pairs.filter(([key]) => inventory.get(store, key) !== undefined).length;
  const rest = kept + found + inventory.room(store) + inventory.limit(store);
  return inventory.total(store) + (inventory.largest(store) ?? 0) + rest;
}
