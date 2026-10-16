const IDLE_TIMEOUT_MS = 2000;
const IDLE_FALLBACK_MS = 300;

export function whenIdle(run: () => void): () => void {
  if (typeof requestIdleCallback === "function") {
    const id = requestIdleCallback(run, { timeout: IDLE_TIMEOUT_MS });
    return () => cancelIdleCallback(id);
  }
  const id = setTimeout(run, IDLE_FALLBACK_MS);
  return () => clearTimeout(id);
}
