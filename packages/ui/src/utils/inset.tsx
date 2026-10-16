import { createContext, useContext, type ReactNode } from "react";

const InsetContext = createContext(false);

export function InsetContent({ children, inset = true }: { children: ReactNode; inset?: boolean | undefined }) {
  return <InsetContext.Provider value={inset}>{children}</InsetContext.Provider>;
}

export function useInset(): boolean {
  return useContext(InsetContext);
}
