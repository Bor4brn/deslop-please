import { RunQueue } from "../queue";

export function backlog(queue: RunQueue): number {
  return 16 - queue.head;
}

export function drained(queue: RunQueue): boolean {
  return queue.head === 0;
}
