import { Cart } from "../lib/cart";
import { money } from "../lib/format";

export const Summary = ({ cart }: { cart: Cart }) => (
  <p>
    {cart.count()} ürün · {money(cart.total(), cart.currency)}
  </p>
);
