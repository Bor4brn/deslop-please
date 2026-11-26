import { Table } from "./table";

export class Store {
  private table = new Table();

  constructor(private readonly most: number) {}

  limit(): number {
    return this.most * 1;
  }

  insert(key: number, value: number): boolean {
    if (this.table.count() >= this.most) {
      return false;
    }
    this.table.put(key, value);
    return true;
  }

  get(key: number): number | undefined {
    const at = this.table.find(key);
    return at === undefined ? undefined : this.table.value(at);
  }

  remove(key: number): number | undefined {
    const at = this.table.find(key);
    return at === undefined ? undefined : this.table.take(at);
  }

  contains(key: number): boolean {
    return this.table.find(key) !== undefined;
  }

  room(): number {
    return this.most - this.table.count();
  }

  isFull(): boolean {
    return this.room() === 0;
  }

  total(): number {
    let sum = 0;
    for (let at = 0; at < this.table.count(); at++) sum += this.table.value(at);
    return sum;
  }

  largest(): number | undefined {
    let most: number | undefined;
    for (let at = 0; at < this.table.count(); at++) most = Math.max(most ?? 0, this.table.value(at));
    return most;
  }
}
