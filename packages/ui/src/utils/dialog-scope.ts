import { createContext, use, useEffect, useId } from "react";

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
