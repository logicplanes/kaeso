import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";

export interface CreateAction {
  label: string;
  run: () => void;
  disabledReason?: string | undefined;
}

const ActionContext = createContext<CreateAction | null>(null);
const SetActionContext = createContext<(action: CreateAction | null) => void>(() => undefined);

export function CreateActionProvider({ children }: { children: ReactNode }) {
  const [action, set] = useState<CreateAction | null>(null);
  return (
    <SetActionContext.Provider value={set}>
      <ActionContext.Provider value={action}>{children}</ActionContext.Provider>
    </SetActionContext.Provider>
  );
}

export function useCreateAction(): CreateAction | null {
  return useContext(ActionContext);
}

export function usePageCreateAction(action: CreateAction | null): void {
  const set = useContext(SetActionContext);
  const latest = useRef(action);
  useEffect(() => {
    latest.current = action;
  });
  const label = action?.label;
  const reason = action?.disabledReason;
  useEffect(() => {
    set(label ? { label, run: () => latest.current?.run(), disabledReason: reason } : null);
    return () => set(null);
  }, [label, reason, set]);
}
