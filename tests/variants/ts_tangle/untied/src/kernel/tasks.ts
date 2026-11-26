export interface Task {
  id: number;
  asleep: boolean;
}

export class Tasks {
  private list: Task[] = [];

  spawn(id: number): void {
    this.list.push({ id, asleep: true });
  }

  wake(id: number): void {
    for (const task of this.list) {
      if (task.id === id) {
        task.asleep = false;
      }
    }
  }

  runnable(): number {
    return this.list.filter((task) => !task.asleep).length;
  }
}
