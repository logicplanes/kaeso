import { forwardRef, type ReactNode } from "react";
import { ScrollArea as BaseScrollArea } from "@base-ui/react/scroll-area";
import { cn } from "../../utils/cn.js";

export const KAESO_SCROLL_AREA_VARIANTS = {
  orientation: {
    vertical: { classes: "scroll-fade-y", description: "Scrolls up and down; the content fades at an edge with more to scroll to (default)" },
    horizontal: { classes: "scroll-fade-x", description: "Scrolls sideways" },
    both: { classes: "", description: "Both directions" },
  },
} as const;
export const KAESO_SCROLL_AREA_DEFAULT_VARIANTS = { orientation: "vertical" } as const;

export interface ScrollAreaProps {
  children: ReactNode;
  orientation?: keyof typeof KAESO_SCROLL_AREA_VARIANTS.orientation | undefined;
  className?: string | undefined;
  viewportClassName?: string | undefined;
  "aria-label"?: string | undefined;
}

const Bar = ({ orientation }: { orientation: "vertical" | "horizontal" }) => (
  <BaseScrollArea.Scrollbar
    orientation={orientation}
    className={cn(
      "flex touch-none p-2 opacity-0 select-none transition-control data-hovering:opacity-100 data-scrolling:opacity-100",
      orientation === "vertical" ? "w-10" : "h-10 flex-col",
    )}
  >
    <BaseScrollArea.Thumb className="flex-1 rounded-pill bg-line-strong" />
  </BaseScrollArea.Scrollbar>
);

export const ScrollArea = forwardRef<HTMLDivElement, ScrollAreaProps>(function ScrollArea(
  { children, orientation = "vertical", className, viewportClassName, "aria-label": label },
  ref,
) {
  return (
    <BaseScrollArea.Root ref={ref} className={cn("relative min-h-0 overflow-hidden", className)}>
      <BaseScrollArea.Viewport
        className={cn("size-full overscroll-contain focus-ring-inset", KAESO_SCROLL_AREA_VARIANTS.orientation[orientation].classes, viewportClassName)}
        {...(label ? { role: "region", "aria-label": label } : {})}
      >
        {children}
      </BaseScrollArea.Viewport>
      {orientation !== "horizontal" && <Bar orientation="vertical" />}
      {orientation !== "vertical" && <Bar orientation="horizontal" />}
      {orientation === "both" && <BaseScrollArea.Corner />}
    </BaseScrollArea.Root>
  );
});
ScrollArea.displayName = "ScrollArea";
