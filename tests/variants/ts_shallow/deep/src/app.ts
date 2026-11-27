import { Store } from "./store";

export function run(pairs: Array<[number, number]>): number {
  const store = new Store(8);
  store.load(pairs);
  return store.summary(pairs.map(([key]) => key));
}
