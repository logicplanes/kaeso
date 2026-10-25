import { createContext, use, useCallback, useState, type ReactNode } from "react";

const PortalCtx = createContext<HTMLElement | null>(null);

export function PortalContainer({ children, className }: { children: ReactNode; className?: string | undefined }) {
  const [el] = useState<HTMLDivElement | null>(() => {
    if (typeof document === "undefined") return null;
    const node = document.createElement("div");
    node.setAttribute("data-portal-container", "");
    return node;
  });
  const attach = useCallback(
    (host: HTMLDivElement | null) => {
      if (host && el && el.parentNode !== host) host.appendChild(el);
    },
    [el],
  );
  return (
    <PortalCtx.Provider value={el}>
      {children}
      <div ref={attach} className={className ?? "absolute top-0 left-0 size-0"} data-portal-host="" />
    </PortalCtx.Provider>
  );
}
PortalContainer.displayName = "PortalContainer";

export function usePortalContainer(): HTMLElement | undefined {
  return use(PortalCtx) ?? undefined;
}

export function pageBoundary(): Element | "clipping-ancestors" {
  return typeof document === "undefined" ? "clipping-ancestors" : document.documentElement;
}
