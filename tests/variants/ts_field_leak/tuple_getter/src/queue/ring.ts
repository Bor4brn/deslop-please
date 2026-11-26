export class RunQueue {
  private items: number[] = [];
  private head = 0;

  constructor(private readonly limit: number) {}

  full(): boolean {
    return this.items.length - this.head >= this.limit;
  }

  parts(): [number[], number] {
    return [this.items, this.head];
  }

  advance(): void {
    this.head += 1;
  }
}
