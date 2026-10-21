import { useRef } from "react";

function focused(): HTMLElement | null {
  if (typeof document === "undefined") return null;
  const a = document.activeElement;
  return a instanceof HTMLElement && a !== document.body ? a : null;
}

function focusedElsewhere(): boolean {
  const layer = focused()?.closest("[role=dialog], [role=alertdialog]");
  return layer === null || !!layer?.hasAttribute("data-open");
}

export function useRestoreFocus(open: boolean | undefined, first: () => HTMLElement | boolean = () => true) {
  const opener = useRef<HTMLElement | null>(null);
  const wasOpen = useRef(false);
  if (open && !wasOpen.current) opener.current = focused();
  if (open !== undefined) wasOpen.current = open;
  return {
    initialFocus: () => first(),
    finalFocus: () => {
      if (open !== undefined && opener.current?.isConnected) return opener.current;
      return !focusedElsewhere();
    },
  };
}
