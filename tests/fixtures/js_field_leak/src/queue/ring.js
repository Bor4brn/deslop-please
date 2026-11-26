export class RunQueue {
  items = [];
  head = 0;
  limit;

  constructor(limit) {
    this.limit = limit;
  }

  full() {
    return this.items.length - this.head >= this.limit;
  }
}
