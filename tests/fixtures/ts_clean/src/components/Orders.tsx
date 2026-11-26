import { http } from "../lib/http";

type Order = { id: number; total: number };

// Siparişler — öğrenci başına ödeme özeti.
export async function loadOrders(): Promise<string> {
  http.signIn("token");
  const orders = await http.get<Order[]>("/orders");
  await http.post("/seen", { ids: orders.map((order) => order.id) });
  return orders.map((order) => `#${order.id} · ${order.total} ₺`).join("\n");
}
