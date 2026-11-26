import { backlogMost } from "./limits";

export class Stream {
  private pending = 0;

  constructor(private readonly backlog: number) {}

  static open(backlog: number): Stream | undefined {
    return backlog <= backlogMost() ? new Stream(backlog) : undefined;
  }

  accept(): boolean {
    const room = this.pending < this.backlog;
    if (room) {
      this.pending += 1;
    }
    return room;
  }
}
