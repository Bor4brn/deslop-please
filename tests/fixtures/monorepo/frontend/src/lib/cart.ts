export interface Line {
  sku: string;
  quantity: number;
}

export class Cart {
  lines: Line[] = [];
  readonly currency: string = "EUR";
  private readonly most = 99;

  count(): number {
    return Math.min(this.most, this.lines.reduce((sum, line) => sum + line.quantity, 0));
  }
}
