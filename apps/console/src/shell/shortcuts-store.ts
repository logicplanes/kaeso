type Listener = () => void;
const listeners = new Set<Listener>();

export function openShortcutSheet(): void {
  for (const l of listeners) l();
}

export function onShortcutSheet(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
