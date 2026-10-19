import { createContext, useContext, type ReactNode } from "react";

const InsetContext = createContext(false);

export function InsetContent({ children, inset = true }: { children: ReactNode; inset?: boolean | undefined }) {
  return <InsetContext.Provider value={inset}>{children}</InsetContext.Provider>;
}

export function useInset(): boolean {
  return useContext(InsetContext);
}

const GutterContext = createContext(false);

export function GutterContent({ children, padded = true }: { children: ReactNode; padded?: boolean | undefined }) {
  return <GutterContext.Provider value={padded}>{children}</GutterContext.Provider>;
}

export function useGutterPadded(): boolean {
  return useContext(GutterContext);
}
