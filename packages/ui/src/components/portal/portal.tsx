import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { usePortalContainer } from "../../utils/portal-context";

export const KAESO_PORTAL_VARIANTS = {} as const;
export const KAESO_PORTAL_DEFAULT_VARIANTS = {} as const;

export interface PortalProps {
  children: ReactNode;
  container?: HTMLElement | null | undefined;
}

export function Portal({ children, container }: PortalProps) {
  const scoped = usePortalContainer();
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  if (!mounted) return null;
  return createPortal(children, container ?? scoped ?? document.body);
}
Portal.displayName = "Portal";
