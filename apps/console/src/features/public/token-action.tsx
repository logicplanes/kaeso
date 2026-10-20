import { useEffect, useEffectEvent, useRef, useState } from "react";
import { toastApiError } from "~/api/toast-error";
import { takeFragment } from "~/auth/fragment";

export type TokenState = { kind: "working" } | { kind: "missing" } | { kind: "failed" } | { kind: "done" };

export function useTokenAction(action: (token: string) => Promise<void>, fragment = "token"): TokenState {
  const [token] = useState(() => takeFragment(fragment));
  const [state, setState] = useState<TokenState>(token ? { kind: "working" } : { kind: "missing" });
  const started = useRef(false);
  const run = useEffectEvent((value: string) => action(value));
  useEffect(() => {
    if (!token || started.current) return;
    started.current = true;
    run(token).then(
      () => setState({ kind: "done" }),
      (e: unknown) => {
        toastApiError(e, { id: "token-link" });
        setState({ kind: "failed" });
      },
    );
  }, [token]);
  return state;
}
