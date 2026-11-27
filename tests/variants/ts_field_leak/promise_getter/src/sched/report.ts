import { RunQueue } from "../queue";

export async function settle(queues: RunQueue[]): Promise<void> {
  await Promise.all(queues.map((queue) => queue.drained()));
}
