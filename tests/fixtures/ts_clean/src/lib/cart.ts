export interface Item {
  sku: string;
  price: number;
}

export class Cart {
  private items: Item[] = [];
  readonly currency: string;

  constructor(currency: string) {
    this.currency = currency;
  }

  add(item: Item): void {
    this.items.push(item);
  }

  total(): number {
    return this.items.reduce((sum, item) => sum + item.price, 0);
  }

  count(): number {
    return this.items.length;
  }
}
