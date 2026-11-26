export class RunQueue {
  #items = [];
  #head = 0;
  #limit;

  constructor(limit) {
    this.#limit = limit;
  }

  push(task) {
    if (this.#items.length - this.#head >= this.#limit) {
      return false;
    }
    this.#items.push(task);
    return true;
  }

  pop() {
    const task = this.#items[this.#head];
    this.#head += 1;
    return task;
  }
}
