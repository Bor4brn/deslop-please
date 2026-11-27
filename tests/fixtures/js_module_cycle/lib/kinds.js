const { BACKLOG_MOST } = require("./socket");

class Stream {
  constructor(backlog) {
    this.backlog = backlog;
  }

  static open(backlog) {
    return backlog <= BACKLOG_MOST ? new Stream(backlog) : undefined;
  }
}

module.exports = { Stream };
