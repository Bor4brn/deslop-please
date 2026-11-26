import { RunQueue } from "../queue/ring.js";

/** @param {RunQueue} queue */
export function admit(queue, task) {
  if (queue.full()) {
    return false;
  }
  queue.items.push(task);
  return true;
}

/** @param {RunQueue} queue */
export function next(queue) {
  const task = queue.items[queue.head];
  queue.head += 1;
  return task;
}
