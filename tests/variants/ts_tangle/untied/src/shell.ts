import { tick } from "./kernel";
import { Tasks } from "./kernel/tasks";
import { Timers } from "./kernel/timers";

export function boot(ids: number[], until: number): number {
  const tasks = new Tasks();
  const timers = new Timers();
  for (const id of ids) {
    tasks.spawn(id);
    timers.arm(id, 10);
  }
  let awake = 0;
  for (let now = 0; now < until; now++) {
    awake = tick(now, tasks, timers);
  }
  return awake;
}
