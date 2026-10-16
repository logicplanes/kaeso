import { forwardRef, type ReactElement, type ReactNode } from "react";
import { PreviewCard as BasePreviewCard } from "@base-ui/react/preview-card";
import { pageBoundary, usePortalContainer } from "../../utils/portal-context";
import { cn } from "../../utils/cn";
import { POPOVER_POPUP, type PopoverContentProps } from "./popover";

export interface HoverCardProps {
  children: ReactNode;
  open?: boolean | undefined;
  defaultOpen?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
}

function HoverRoot({ children, open, defaultOpen, onOpenChange }: HoverCardProps) {
  return (
    <BasePreviewCard.Root
      {...(open !== undefined ? { open } : {})}
      {...(defaultOpen !== undefined ? { defaultOpen } : {})}
      {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}
    >
      {children}
    </BasePreviewCard.Root>
  );
}
HoverRoot.displayName = "HoverCard";
function HoverTrigger({ children, delay = 600 }: { children: ReactElement; delay?: number }) {
  return <BasePreviewCard.Trigger render={children} delay={delay} closeDelay={150} />;
}
HoverTrigger.displayName = "HoverCard.Trigger";
const HoverContent = forwardRef<HTMLDivElement, Omit<PopoverContentProps, "title" | "padding">>(function HoverCardContent(
  { children, side = "bottom", align = "start", sideOffset = 8, className },
  ref,
) {
  return (
    <BasePreviewCard.Portal container={usePortalContainer()}>
      <BasePreviewCard.Positioner
        positionMethod="fixed"
        collisionBoundary={pageBoundary()}
        side={side}
        align={align}
        sideOffset={sideOffset}
        collisionPadding={8}
        className="z-50"
      >
        <BasePreviewCard.Popup ref={ref} className={cn(POPOVER_POPUP, "w-320 p-16", className)}>
          {children}
        </BasePreviewCard.Popup>
      </BasePreviewCard.Positioner>
    </BasePreviewCard.Portal>
  );
});
HoverContent.displayName = "HoverCard.Content";
export const HoverCard = Object.assign(HoverRoot, { Trigger: HoverTrigger, Content: HoverContent });
