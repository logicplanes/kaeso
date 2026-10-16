const taken = new Map<string, string>();

function entryKey(): string {
  const state: unknown = window.history.state;
  return state && typeof state === "object" && "__TSR_key" in state ? String(state.__TSR_key) : "";
}

export function takeFragment(name: string): string | null {
  const params = new URLSearchParams(window.location.hash.slice(1));
  const value = params.get(name);
  const key = `${entryKey()}${window.location.pathname}#${name}`;
  if (value !== null) {
    taken.set(key, value);
    window.history.replaceState(window.history.state, "", window.location.pathname + window.location.search);
  }
  return taken.get(key) ?? null;
}
