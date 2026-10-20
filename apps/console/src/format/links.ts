export function httpsHref(href: string): string | null {
  try {
    const url = new URL(href.trim());
    return url.protocol === "https:" && url.username === "" && url.password === "" ? url.href : null;
  } catch {
    return null;
  }
}
