export class RunQueue {
  private slots = new Slots();

  constructor(private readonly limit: number) {}

  full(): boolean {
    return this.slots.waiting() >= this.limit;
  }

  push(task: number): void {
    this.slots.push(task);
  }

  pop(): number | undefined {
    return this.slots.pop();
  }

  waiting(): number {
    return this.slots.waiting();
  }
}

class Slots {
  private items: number[] = [];
  private head = 0;

  push(task: number): void {
    this.items.push(task);
  }

  pop(): number | undefined {
    const task = this.items[this.head];
    if (task !== undefined) {
      this.head += 1;
    }
    return task;
  }

  waiting(): number {
    return this.items.length - this.head;
  }
}
