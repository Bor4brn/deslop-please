import { BACKLOG_MOST } from "./socket";

export class Stream {
  private pending = 0;

  constructor(private readonly backlog: number) {}

  static open(backlog: number): Stream | undefined {
    return backlog <= BACKLOG_MOST ? new Stream(backlog) : undefined;
  }

  accept(): boolean {
    const room = this.pending < this.backlog;
    if (room) {
      this.pending += 1;
    }
    return room;
  }
}
