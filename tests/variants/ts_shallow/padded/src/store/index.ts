import { Table } from "./table";

export class Store {
  private table = new Table();

  constructor(private readonly most: number) {}

  limit(): number {
    return this.most * 1;
  }

  private tables(): Table {
    return this.table;
  }

  private count(): number {
    return this.table.count();
  }

  private find(key: number): number | undefined {
    return this.tables().find(key);
  }

  private value(at: number): number {
    return this.tables().value(at);
  }

  insert(key: number, value: number): boolean {
    if (this.count() >= this.limit()) {
      return false;
    }
    this.tables().put(key, value);
    return true;
  }

  get(key: number): number | undefined {
    const at = this.find(key);
    return at === undefined ? undefined : this.value(at);
  }

  remove(key: number): number | undefined {
    const at = this.find(key);
    return at === undefined ? undefined : this.tables().take(at);
  }

  contains(key: number): boolean {
    return this.find(key) !== undefined;
  }

  room(): number {
    return this.limit() - this.count();
  }

  isFull(): boolean {
    return this.room() === 0;
  }

  total(): number {
    let sum = 0;
    for (let at = 0; at < this.count(); at++) sum += this.value(at);
    return sum;
  }

  largest(): number | undefined {
    let most: number | undefined;
    for (let at = 0; at < this.count(); at++) most = Math.max(most ?? 0, this.value(at));
    return most;
  }
}
