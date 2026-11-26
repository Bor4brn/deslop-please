import { Stream } from "./kinds";

export function listen(backlog: number): Stream | undefined {
  return Stream.open(backlog);
}
