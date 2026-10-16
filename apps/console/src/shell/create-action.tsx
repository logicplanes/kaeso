import { createContext, useContext, useEffect, useEffectEvent, useState, type ReactNode } from "react";

export interface CreateAction {
  label: string;
  run: () => void;
  disabledReason?: string | undefined;
}

const Ctx = createContext<{ action: CreateAction | null; set: (a: CreateAction | null) => void }>({ action: null, set: () => undefined });

export function CreateActionProvider({ children }: { children: ReactNode }) {
  const [action, set] = useState<CreateAction | null>(null);
  return <Ctx.Provider value={{ action, set }}>{children}</Ctx.Provider>;
}

export function useCreateAction(): CreateAction | null {
  return useContext(Ctx).action;
}

export function usePageCreateAction(action: CreateAction | null): void {
  const { set } = useContext(Ctx);
  const run = useEffectEvent(() => action?.run());
  const label = action?.label;
  const reason = action?.disabledReason;
  useEffect(() => {
    set(label ? { label, run, disabledReason: reason } : null);
    return () => set(null);
  }, [label, reason, set]);
}
