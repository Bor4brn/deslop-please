export class Timers {
  private due: Array<[number, number]> = [];

  arm(id: number, at: number): void {
    this.due.push([id, at]);
  }

  fire(now: number): number[] {
    const fired = this.due.filter(([, at]) => at <= now).map(([id]) => id);
    this.due = this.due.filter(([, at]) => at > now);
    return fired;
  }
}
