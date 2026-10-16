const early: KeyboardEvent[] = [];
const keep = (e: KeyboardEvent) => void early.push(e);
window.addEventListener("keydown", keep);

export function takeEarlyKeys(): KeyboardEvent[] {
  window.removeEventListener("keydown", keep);
  return early.splice(0);
}
