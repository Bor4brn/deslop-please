/** The most tasks a queue holds. */
export class Limit {
  constructor(readonly most: number) {}
}

export class RunQueue {
  private items: number[] = [];
  head = 0;
  private readonly most: Limit;

  constructor(limit: number) {
    this.most = new Limit(limit);
  }

  push(task: number): boolean {
    if (this.backlog() >= this.most.most) {
      return false;
    }
    this.items.push(task);
    return true;
  }

  pop(): number | undefined {
    const task = this.items[this.head];
    if (task !== undefined) {
      this.head += 1;
    }
    return task;
  }

  backlog(): number {
    return this.items.length - this.head;
  }

  limit(): Limit {
    return this.most;
  }
}
