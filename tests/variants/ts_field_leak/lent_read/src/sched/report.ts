import { RunQueue } from "../queue";

export function backlog(queue: RunQueue): number {
  return Math.min(queue.backlog(), queue.limit().most);
}

export function drained(queue: RunQueue): boolean {
  return queue.backlog() === 0 || queue.head === queue.limit().most;
}
