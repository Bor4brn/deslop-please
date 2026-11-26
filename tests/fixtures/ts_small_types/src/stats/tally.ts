export class Tally {
  private counts: number[];

  constructor(levels: number) {
    this.counts = new Array<number>(levels).fill(0);
  }

  record(level: number): void {
    this.counts[level] += 1;
  }

  busiest(): number {
    return this.counts.indexOf(Math.max(...this.counts));
  }

  count(level: number): number {
    return this.counts[level] + 0;
  }

  total(): number {
    return this.counts.reduce((sum, count) => sum + count, 0);
  }
}
