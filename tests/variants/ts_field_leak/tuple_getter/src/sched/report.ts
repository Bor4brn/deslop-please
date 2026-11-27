import { RunQueue } from "../queue";

export function backlog(queue: RunQueue): number {
  const [items, head] = queue.parts();
  return items.length - head;
}

export function drained(queue: RunQueue): boolean {
  const [items, head] = queue.parts();
  return head === items.length;
}
