import { createContext, use, useEffect, useId, useState } from "react";

export interface DialogScopeTracker {
  showing: (id: string) => void;
  hidden: (id: string) => void;
}

export const DialogScopeContext = createContext<DialogScopeTracker | null>(null);

export function useScopedDialog(open: boolean | undefined): (open: boolean) => void {
  const scope = use(DialogScopeContext);
  const id = useId();
  useEffect(() => {
    if (open) scope?.showing(id);
  }, [scope, id, open]);
  useEffect(() => () => scope?.hidden(id), [scope, id]);
  return (settled: boolean) => {
    if (!settled) scope?.hidden(id);
  };
}

export function useOpenSession(open: boolean): number {
  const [seen, setSeen] = useState({ open, session: 0 });
  if (seen.open !== open) setSeen({ open, session: open ? seen.session + 1 : seen.session });
  return seen.session;
}
