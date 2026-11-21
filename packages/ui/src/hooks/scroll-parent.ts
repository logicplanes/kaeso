export function scrollParent(el: HTMLElement | null): HTMLElement | null {
  // A virtualizer asks for its scroll position while rendering, which a server does too, with no document to scroll.
  if (typeof document === "undefined") return null;
  let p = el?.parentElement ?? null;
  while (p) {
    const s = getComputedStyle(p);
    if (/(auto|scroll|overlay)/.test(s.overflowY) && p.scrollHeight > p.clientHeight) return p;
    p = p.parentElement;
  }
  return (document.scrollingElement as HTMLElement | null) ?? null;
}
