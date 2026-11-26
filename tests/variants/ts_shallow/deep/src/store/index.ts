import { Table } from "./table";

export class Store {
  private table = new Table();

  constructor(private readonly most: number) {}

  load(pairs: Array<[number, number]>): void {
    for (const [key, value] of pairs) {
      if (this._isFull() || !this._insert(key, value)) {
        this._remove(key);
      }
    }
  }

  summary(keys: number[]): number {
    const kept = keys.filter((key) => this._contains(key)).length;
    const found = keys.filter((key) => this._get(key) !== undefined).length;
    const rest = kept + found + this._room() + this.most;
    return this._total() + (this._largest() ?? 0) + rest;
  }

  private _insert(key: number, value: number): boolean {
    if (this.table.count() >= this.most) {
      return false;
    }
    this.table.put(key, value);
    return true;
  }

  private _get(key: number): number | undefined {
    const at = this.table.find(key);
    return at === undefined ? undefined : this.table.value(at);
  }

  private _remove(key: number): number | undefined {
    const at = this.table.find(key);
    return at === undefined ? undefined : this.table.take(at);
  }

  private _contains(key: number): boolean {
    return this.table.find(key) !== undefined;
  }

  private _room(): number {
    return this.most - this.table.count();
  }

  private _isFull(): boolean {
    return this._room() === 0;
  }

  private _total(): number {
    let sum = 0;
    for (let at = 0; at < this.table.count(); at++) sum += this.table.value(at);
    return sum;
  }

  private _largest(): number | undefined {
    let most: number | undefined;
    for (let at = 0; at < this.table.count(); at++) most = Math.max(most ?? 0, this.table.value(at));
    return most;
  }
}
