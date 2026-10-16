type Listener = (open: boolean) => void;

let pending: Promise<boolean> | null = null;
let resolvePending: ((ok: boolean) => void) | null = null;
const listeners = new Set<Listener>();

export function requestRecentSignIn(): Promise<boolean> {
  if (!pending) {
    pending = new Promise<boolean>((resolve) => {
      resolvePending = resolve;
    });
    for (const l of listeners) l(true);
  }
  return pending;
}

export function settleRecentSignIn(ok: boolean): void {
  resolvePending?.(ok);
  pending = null;
  resolvePending = null;
  for (const l of listeners) l(false);
}

export function onRecentSignInChange(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export const RETURN_KEY = "kaeso:return";
