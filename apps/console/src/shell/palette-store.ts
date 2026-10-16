type Listener = (open: boolean) => void;
const listeners = new Set<Listener>();

export function openPalette(): void {
  for (const l of listeners) l(true);
}

export function onPaletteOpen(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

let shown = false;

export const isPaletteOpen = () => shown;

export function setPaletteShown(open: boolean): void {
  shown = open;
}
