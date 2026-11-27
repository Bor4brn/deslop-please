import { Tasks } from "./tasks";
import { Timers } from "./timers";

export function tick(now: number, tasks: Tasks, timers: Timers): number {
  for (const id of timers.fire(now)) {
    tasks.wake(id);
  }
  return tasks.runnable();
}
