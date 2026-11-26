import { Tasks } from "./tasks";

export class Timers {
  private due: Array<[number, number]> = [];

  arm(id: number, at: number): void {
    this.due.push([id, at]);
  }

  fire(now: number, tasks: Tasks): void {
    for (const [id, at] of this.due) {
      if (at <= now) {
        tasks.wake(id);
      }
    }
    this.due = this.due.filter(([, at]) => at > now);
  }
}
