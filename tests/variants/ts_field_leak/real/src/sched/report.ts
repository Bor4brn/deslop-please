import { RunQueue } from "../queue";

export function backlog(queue: RunQueue): number {
  return queue.backlog();
}

export function drained(queue: RunQueue): boolean {
  return queue.backlog() === 0;
}
