const kinds = require("./kinds");

const BACKLOG_MOST = 16;

function listen(backlog) {
  return kinds.Stream.open(backlog);
}

module.exports = { BACKLOG_MOST, listen };
