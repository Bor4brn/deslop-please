export class RunQueue {
  private items: number[] = [];
  private head = 0;
  private settled: Promise<void> = Promise.resolve();

  constructor(private readonly limit: number) {}

  push(task: number): boolean {
    if (this.items.length - this.head >= this.limit) {
      return false;
    }
    this.items.push(task);
    this.settled = this.settled.then(() => undefined);
    return true;
  }

  pop(): number | undefined {
    const task = this.items[this.head];
    if (task !== undefined) {
      this.head += 1;
    }
    return task;
  }

  /** What is queued so far, settled. */
  drained(): Promise<void> {
    return this.settled;
  }
}
