export async function createUser(id: number, name: string): Promise<unknown> {
  const response = await fetch(`/api/users/${id}?name=${encodeURIComponent(name)}`, { method: "POST" });
  return response.json();
}
