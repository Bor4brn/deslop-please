export class RunQueue {
  private items: number[] = [];
  private head = 0;

  constructor(private readonly limit: number) {}

  push(task: number): boolean {
    if (this.backlog() >= this.limit) {
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
}
