export class Window {
  private latencies: number[] = [];

  constructor(private readonly size: number) {}

  push(latency: number): void {
    this.latencies = [...this.latencies, latency].slice(-this.size);
  }

  mean(): number {
    return Math.floor(this.latencies.reduce((sum, l) => sum + l, 0) / Math.max(this.latencies.length, 1));
  }

  worst(): number {
    return Math.max(0, ...this.latencies);
  }
}
