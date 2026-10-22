const BASE = "https://console.invalid";

export function internalPath(path: string | null | undefined): string | null {
  if (!path?.startsWith("/")) return null;
  const url = new URL(path, BASE);
  if (url.origin !== BASE || url.pathname.startsWith("//") || url.pathname.startsWith("/\\")) return null;
  return `${url.pathname}${url.search}${url.hash}`;
}

export function safeNext(next: string | undefined): string {
  return internalPath(next) ?? "/";
}
