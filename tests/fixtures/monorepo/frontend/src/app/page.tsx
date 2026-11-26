import { Cart } from "../lib/cart";
import { createUser } from "../lib/api";

const cart = new Cart();

export default function Page() {
  cart.lines.push({ sku: "tea", quantity: 1 });
  void createUser(1, "ada");
  return (
    <main>
      {cart.count()} items in {cart.currency}
    </main>
  );
}
