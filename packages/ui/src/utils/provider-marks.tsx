import { createContext, useContext, type ReactNode } from "react";

type MarkOf = (provider: string) => string | null | undefined;

const ProviderMarksContext = createContext<MarkOf>(() => undefined);

export function ProviderMarksProvider({ markOf, children }: { markOf: MarkOf; children: ReactNode }) {
  return <ProviderMarksContext.Provider value={markOf}>{children}</ProviderMarksContext.Provider>;
}
ProviderMarksProvider.displayName = "ProviderMarksProvider";

export function useProviderMarkOf(): MarkOf {
  return useContext(ProviderMarksContext);
}
