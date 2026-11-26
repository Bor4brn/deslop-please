// İstemci: her istek tek kapıdan geçer · yanıt gövdesi çözülür — “ok”.
type Request = { path: string; method: string };

export class Http {
  private token: string | null = null;

  get = <T>(path: string) => this.send<T>({ path, method: "GET" });
  post = async <T>(path: string, body: unknown): Promise<T> =>
    this.send<T>({ path, method: "POST" }, body);

  signIn(token: string): void {
    this.token = token.trim();
  }

  private async send<T>(request: Request, body?: unknown): Promise<T> {
    const headers = this.token ? { authorization: `Bearer ${this.token}` } : {};
    const response = await fetch(request.path, { method: request.method, headers, body: JSON.stringify(body) });
    return (await response.json()) as T;
  }
}

export const http = new Http();
