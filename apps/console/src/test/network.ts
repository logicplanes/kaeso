export interface ApiRequest {
  method: string;
  path: string;
  url: URL;
  params: Record<string, string>;
  query: URLSearchParams;
  headers: Headers;
  body: unknown;
}

export type Handler = (request: ApiRequest) => unknown;

interface Route {
  method: string;
  pattern: RegExp;
  names: string[];
  handler: Handler;
}

const routes: Route[] = [];
const seen: ApiRequest[] = [];

const API = "/platform/v1";

function compile(path: string): { pattern: RegExp; names: string[] } {
  const names: string[] = [];
  const source = path
    .split("/")
    .map((part) => {
      if (!part.startsWith(":")) return part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      names.push(part.slice(1));
      return "([^/]+)";
    })
    .join("/");
  return { pattern: new RegExp(`^${source}$`), names };
}

export function json(body: unknown, init: { status?: number; headers?: Record<string, string> } = {}): Response {
  return new Response(JSON.stringify(body), { status: init.status ?? 200, headers: { "content-type": "application/json", ...init.headers } });
}

export function apiError(status: number, type: string, message: string, extra: { param?: string; right?: string } = {}): Response {
  return json({ error: { type, message, param: extra.param ?? null, right: extra.right ?? null } }, { status });
}

export function noContent(): Response {
  return new Response(null, { status: 204 });
}

export function page<T>(data: T[], next: string | null = null): { data: T[]; next_cursor: string | null } {
  return { data, next_cursor: next };
}

export function on(method: string, path: string, reply: Handler | unknown): void {
  const { pattern, names } = compile(path.startsWith("/") ? path : `${API}/${path}`);
  const handler: Handler = typeof reply === "function" ? (reply as Handler) : () => reply;
  routes.unshift({ method: method.toUpperCase(), pattern, names, handler });
}

export function requests(method?: string, path?: string): ApiRequest[] {
  if (!method) return [...seen];
  const target = path ? compile(path.startsWith("/") ? path : `${API}/${path}`).pattern : null;
  return seen.filter((r) => r.method === method.toUpperCase() && (!target || target.test(r.path)));
}

export function resetNetwork(): void {
  routes.length = 0;
  seen.length = 0;
}

async function bodyOf(request: Request): Promise<unknown> {
  const text = await request.text();
  if (!text) return undefined;
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return text;
  }
}

export async function fetchStub(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  const request = new Request(input instanceof URL ? input.href : input, init);
  const url = new URL(request.url, window.location.href);
  const method = request.method.toUpperCase();
  for (const route of routes) {
    if (route.method !== method) continue;
    const match = route.pattern.exec(url.pathname);
    if (!match) continue;
    const params = Object.fromEntries(route.names.map((name, i) => [name, decodeURIComponent(match[i + 1] ?? "")]));
    const apiRequest: ApiRequest = { method, path: url.pathname, url, params, query: url.searchParams, headers: request.headers, body: await bodyOf(request) };
    seen.push(apiRequest);
    const reply = await route.handler(apiRequest);
    if (reply instanceof Response) return reply;
    if (reply === undefined) return noContent();
    return json(reply);
  }
  return apiError(404, "not_found", `No test handler for ${method} ${url.pathname}`);
}
