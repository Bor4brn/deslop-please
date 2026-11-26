import { Stream } from "./kinds";

export const BACKLOG_MOST = 16;

export function listen(backlog: number): Stream | undefined {
  return Stream.open(backlog);
}
