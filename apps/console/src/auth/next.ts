export function internalPath(path: string | null | undefined): string | null {
  return path && path.startsWith("/") && !path.startsWith("//") ? path : null;
}

export function safeNext(next: string | undefined): string {
  return internalPath(next) ?? "/";
}
