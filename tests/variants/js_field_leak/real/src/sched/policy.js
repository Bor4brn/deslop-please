import { RunQueue } from "../queue/ring.js";

/** @param {RunQueue} queue */
export function admit(queue, task) {
  return queue.push(task);
}

/** @param {RunQueue} queue */
export function next(queue) {
  return queue.pop();
}
