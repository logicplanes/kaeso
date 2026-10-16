import { useEffect, useEffectEvent, useRef, useState } from "react";
import { isApiError } from "~/api/errors";
import { takeFragment } from "~/auth/fragment";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

export type TokenState = { kind: "working" } | { kind: "missing" } | { kind: "failed" } | { kind: "done" };

export function useTokenAction(action: (token: string) => Promise<void>): TokenState {
  const [token] = useState(() => takeFragment("token"));
  const [state, setState] = useState<TokenState>(token ? { kind: "working" } : { kind: "missing" });
  const started = useRef(false);
  const run = useEffectEvent((value: string) => action(value));
  useEffect(() => {
    if (!token || started.current) return;
    started.current = true;
    run(token).then(
      () => setState({ kind: "done" }),
      (e: unknown) => {
        toast.error(isApiError(e) ? e.message : t("common.unreachable", "Can't reach the gateway. Try again."), { id: "token-link" });
        setState({ kind: "failed" });
      },
    );
  }, [token]);
  return state;
}
