export class RunQueue {
  constructor(limit) {
    this.items = [];
    this.head = 0;
    this.limit = limit;
  }

  full() {
    return this.items.length - this.head >= this.limit;
  }
}
