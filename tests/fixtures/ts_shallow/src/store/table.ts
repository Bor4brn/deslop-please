export class Table {
  private keys: number[] = [];
  private values: number[] = [];

  count(): number {
    return this.keys.length;
  }

  put(key: number, value: number): void {
    this.keys.push(key);
    this.values.push(value);
  }

  find(key: number): number | undefined {
    const at = this.keys.indexOf(key);
    return at < 0 ? undefined : at;
  }

  value(at: number): number {
    return this.values[at];
  }

  take(at: number): number {
    this.keys.splice(at, 1);
    return this.values.splice(at, 1)[0];
  }
}
