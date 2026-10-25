import { Fragment, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { DialogScopeContext, type DialogScopeTracker } from "../../utils/dialog-scope.js";

export const KAESO_DIALOG_SCOPE_VARIANTS = {} as const;
export const KAESO_DIALOG_SCOPE_DEFAULT_VARIANTS = {} as const;

export interface DialogScopeProps {
  open: boolean;
  children: ReactNode;
}

interface ScopeState {
  open: boolean;
  session: number;
  mounted: boolean;
}

const unmountClosed = (s: ScopeState): ScopeState => (s.open || !s.mounted ? s : { ...s, mounted: false });

export function DialogScope({ open, children }: DialogScopeProps) {
  const [state, setState] = useState<ScopeState>({ open, session: 0, mounted: open });
  if (state.open !== open) setState({ open, session: open ? state.session + 1 : state.session, mounted: open || state.mounted });
  const showing = useRef(new Set<string>());
  const closed = useRef(!open);
  useEffect(() => {
    closed.current = !open;
    if (!open && showing.current.size === 0) setState(unmountClosed);
  }, [open]);
  const tracker = useMemo<DialogScopeTracker>(
    () => ({
      showing: (id) => void showing.current.add(id),
      hidden: (id) => {
        showing.current.delete(id);
        if (closed.current && showing.current.size === 0) setState(unmountClosed);
      },
    }),
    [],
  );
  if (!state.mounted) return null;
  return (
    <DialogScopeContext.Provider value={tracker}>
      <Fragment key={state.session}>{children}</Fragment>
    </DialogScopeContext.Provider>
  );
}
DialogScope.displayName = "DialogScope";
