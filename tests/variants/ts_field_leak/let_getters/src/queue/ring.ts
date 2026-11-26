export class RunQueue {
  private _items: number[] = [];
  private _head = 0;

  constructor(private readonly limit: number) {}

  full(): boolean {
    return this._items.length - this._head >= this.limit;
  }

  items(): number[] {
    const items = this._items;
    return items;
  }

  withItems<R>(each: (items: number[]) => R): R {
    return each(this._items);
  }

  head(): number {
    const head = this._head;
    return head;
  }

  setHead(head: number): void {
    this._head = head;
  }
}
