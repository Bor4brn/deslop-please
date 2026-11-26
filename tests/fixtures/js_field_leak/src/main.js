import { RunQueue } from "./queue/ring.js";
import { admit, next } from "./sched/policy.js";

const queue = new RunQueue(4);
admit(queue, 1);
console.log(next(queue));
