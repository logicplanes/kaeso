import { forwardRef, useEffect, useMemo, useRef, type ReactNode, type RefObject } from "react";
import { Dialog } from "@base-ui/react/dialog";
import { usePortalContainer } from "../../utils/portal-context.js";
import { SidebarFloatingContext, SidebarSheetContext } from "../sidebar/sidebar.js";
import { useUiText } from "../../i18n/context.js";

export interface NavSheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  peek?: boolean | undefined;
  returnFocus?: RefObject<HTMLElement | null> | undefined;
  children: ReactNode;
}

export const NavSheet = forwardRef<HTMLDivElement, NavSheetProps>(function NavSheet({ open, onOpenChange, peek = false, returnFocus, children }, ref) {
  const leave = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const t = useUiText();
  useEffect(() => () => clearTimeout(leave.current), []);
  const sheet = useMemo(() => ({ close: () => onOpenChange(false), peek }), [onOpenChange, peek]);
  useEffect(() => {
    if (!open || peek || typeof window === "undefined") return;
    const wide = window.matchMedia("(min-width: 1024px)");
    const onChange = () => wide.matches && onOpenChange(false);
    onChange();
    wide.addEventListener("change", onChange);
    return () => wide.removeEventListener("change", onChange);
  }, [open, peek, onOpenChange]);
  return (
    <Dialog.Root open={open} onOpenChange={(o: boolean) => onOpenChange(o)} modal={!peek}>
      <Dialog.Portal container={usePortalContainer()}>
        <Dialog.Backdrop className="fixed inset-0 z-40 bg-overlay opacity-60 backdrop-motion" />
        <Dialog.Popup
          ref={ref}
          aria-label={t("appFrame.navigation", "Navigation")}
          data-side="left"
          initialFocus={!peek}
          {...(returnFocus ? { finalFocus: returnFocus } : {})}
          className="fixed top-0 bottom-0 left-0 z-40 flex overflow-hidden rounded-r-sheet bg-chrome shadow-dialog outline-none safe-area-sheet sheet-motion"
          onClick={(e) => {
            if ((e.target as HTMLElement).closest("a[href]")) onOpenChange(false);
          }}
          onPointerEnter={() => clearTimeout(leave.current)}
          onPointerLeave={() => {
            if (!peek) return;
            leave.current = setTimeout(() => onOpenChange(false), 160);
          }}
        >
          <SidebarFloatingContext.Provider value>
            <SidebarSheetContext.Provider value={sheet}>{children}</SidebarSheetContext.Provider>
          </SidebarFloatingContext.Provider>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
});
NavSheet.displayName = "NavSheet";
