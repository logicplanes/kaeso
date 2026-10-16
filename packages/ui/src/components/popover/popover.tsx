import { InsetContent } from "../../utils/inset.js";
import { pageBoundary, usePortalContainer } from "../../utils/portal-context.js";
import { forwardRef, type ReactNode } from "react";
import { Popover as BasePopover } from "@base-ui/react/popover";
import { cn } from "../../utils/cn.js";
import { PopoverClose, PopoverRoot, PopoverTrigger } from "./popover-root.js";

export type { PopoverProps } from "./popover-root.js";

export const KAESO_POPOVER_VARIANTS = {
  padding: {
    form: { classes: "p-12", description: "12px: forms and free content (default)" },
    list: { classes: "p-6", description: "6px: lists of rows, concentric with 10px items" },
    none: { classes: "p-0", description: "No padding: calendars and custom layouts" },
  },
} as const;
export const KAESO_POPOVER_DEFAULT_VARIANTS = { padding: "form" } as const;

type Side = "top" | "bottom" | "left" | "right";
type Align = "start" | "center" | "end";

export const POPOVER_POPUP =
  "z-50 rounded-popover bg-raised text-body text-fg shadow-raised outline-none popup-motion max-h-(--available-height) overflow-auto scrollbar-quiet";

export interface PopoverContentProps {
  children: ReactNode;
  side?: Side | undefined;
  align?: Align | undefined;
  sideOffset?: number | undefined;
  padding?: keyof typeof KAESO_POPOVER_VARIANTS.padding | undefined;
  title?: ReactNode | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

const Content = forwardRef<HTMLDivElement, PopoverContentProps>(function PopoverContent(
  { children, side = "bottom", align = "start", sideOffset = 6, padding = "form", title, className, "aria-label": ariaLabel },
  ref,
) {
  return (
    <BasePopover.Portal container={usePortalContainer()}>
      <BasePopover.Positioner
        positionMethod="fixed"
        collisionBoundary={pageBoundary()}
        side={side}
        align={align}
        sideOffset={sideOffset}
        collisionPadding={8}
        className="z-50 outline-none"
      >
        <BasePopover.Popup
          ref={ref}
          {...(ariaLabel ? { "aria-label": ariaLabel } : {})}
          className={cn(POPOVER_POPUP, KAESO_POPOVER_VARIANTS.padding[padding].classes, className)}
        >
          {title && <BasePopover.Title className="mb-8 text-body font-semibold text-fg">{title}</BasePopover.Title>}
          <InsetContent inset={false}>{children}</InsetContent>
        </BasePopover.Popup>
      </BasePopover.Positioner>
    </BasePopover.Portal>
  );
});
Content.displayName = "Popover.Content";

export const Popover = Object.assign(PopoverRoot, { Trigger: PopoverTrigger, Content, Close: PopoverClose });
Popover.displayName = "Popover";
