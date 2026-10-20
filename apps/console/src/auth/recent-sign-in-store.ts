type Listener = (open: boolean) => void;

export type SignInAgain = "recent" | "without-sso";

let pending: Promise<boolean> | null = null;
let resolvePending: ((ok: boolean) => void) | null = null;
let current: SignInAgain = "recent";
const listeners = new Set<Listener>();

export function requestRecentSignIn(kind: SignInAgain = "recent"): Promise<boolean> {
  if (!pending) {
    current = kind;
    pending = new Promise<boolean>((resolve) => {
      resolvePending = resolve;
    });
    for (const l of listeners) l(true);
  }
  return pending;
}

export function signInAgainKind(): SignInAgain {
  return current;
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
