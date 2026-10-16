import type { ReactElement, ReactNode } from "react";
import { Popover as BasePopover } from "@base-ui/react/popover";

export interface PopoverProps {
  children: ReactNode;
  open?: boolean | undefined;
  defaultOpen?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  trapFocus?: boolean | undefined;
}

export function PopoverRoot({ open, defaultOpen, onOpenChange, trapFocus, children }: PopoverProps) {
  return (
    <BasePopover.Root
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}
      modal={trapFocus ? "trap-focus" : false}
    >
      {children}
    </BasePopover.Root>
  );
}
PopoverRoot.displayName = "Popover";

export function PopoverTrigger({ children }: { children: ReactElement }) {
  return <BasePopover.Trigger render={children} />;
}
PopoverTrigger.displayName = "Popover.Trigger";

export function PopoverClose({ children }: { children: ReactElement }) {
  return <BasePopover.Close render={children} />;
}
PopoverClose.displayName = "Popover.Close";
