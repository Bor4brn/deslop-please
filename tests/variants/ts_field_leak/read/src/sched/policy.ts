import { RunQueue } from "../queue";

export function admit(queue: RunQueue, task: number): boolean {
  return !queue.full() && task >= queue.head;
}

export function next(queue: RunQueue): number | undefined {
  return queue.head > 0 ? queue.head : undefined;
}
