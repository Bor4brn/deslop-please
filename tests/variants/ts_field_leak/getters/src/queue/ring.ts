export class RunQueue {
  private _items: number[] = [];
  private _head = 0;

  constructor(private readonly limit: number) {}

  get items(): number[] {
    return this._items;
  }

  get head(): number {
    return this._head;
  }

  set head(head: number) {
    this._head = head;
  }

  full(): boolean {
    return this._items.length - this._head >= this.limit;
  }
}
