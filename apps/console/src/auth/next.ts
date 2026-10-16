const BASE = "https://console.invalid";

export function internalPath(path: string | null | undefined): string | null {
  if (!path?.startsWith("/")) return null;
  const url = new URL(path, BASE);
  return url.origin === BASE ? `${url.pathname}${url.search}${url.hash}` : null;
}

export function safeNext(next: string | undefined): string {
  return internalPath(next) ?? "/";
}
