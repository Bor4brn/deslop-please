import { Table } from "./table";

export class Store {
  table = new Table();

  constructor(readonly most: number) {}
}

export function limit(store: Store): number {
  return store.most * 1;
}

export function insert(store: Store, key: number, value: number): boolean {
  if (store.table.count() >= store.most) {
    return false;
  }
  store.table.put(key, value);
  return true;
}

export function get(store: Store, key: number): number | undefined {
  const at = store.table.find(key);
  return at === undefined ? undefined : store.table.value(at);
}

export function remove(store: Store, key: number): number | undefined {
  const at = store.table.find(key);
  return at === undefined ? undefined : store.table.take(at);
}

export function contains(store: Store, key: number): boolean {
  return store.table.find(key) !== undefined;
}

export function room(store: Store): number {
  return store.most - store.table.count();
}

export function isFull(store: Store): boolean {
  return room(store) === 0;
}

export function total(store: Store): number {
  let sum = 0;
  for (let at = 0; at < store.table.count(); at++) sum += store.table.value(at);
  return sum;
}

export function largest(store: Store): number | undefined {
  let most: number | undefined;
  for (let at = 0; at < store.table.count(); at++) most = Math.max(most ?? 0, store.table.value(at));
  return most;
}
