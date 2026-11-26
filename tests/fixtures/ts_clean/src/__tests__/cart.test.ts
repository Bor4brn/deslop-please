import { Cart } from "../lib/cart";

const cart = new Cart("EUR");
cart.add({ sku: "a", price: 2 });
(cart as unknown as { items: unknown[] }).items.length = 0;
