import { RunQueue } from "../queue";

export function admit(queue: RunQueue, task: number): boolean {
  if (queue.full()) {
    return false;
  }
  const [items] = queue.parts();
  items.push(task);
  return true;
}

export function next(queue: RunQueue): number | undefined {
  const [items, head] = queue.parts();
  queue.advance();
  return items[head];
}
