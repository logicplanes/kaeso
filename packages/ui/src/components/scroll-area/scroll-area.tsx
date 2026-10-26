import { forwardRef, type ReactNode } from "react";
import { CSPProvider } from "@base-ui/react/csp-provider";
import { ScrollArea as BaseScrollArea } from "@base-ui/react/scroll-area";
import { cn } from "../../utils/cn.js";

export const KAESO_SCROLL_AREA_VARIANTS = {
  orientation: {
    vertical: { classes: "scroll-axis-y scroll-fade-y", description: "Scrolls up and down; the content fades at an edge with more to scroll to (default)" },
    horizontal: { classes: "scroll-axis-x scroll-fade-x", description: "Scrolls sideways" },
    both: { classes: "scroll-axis-both scroll-fade-both", description: "Both directions; the content fades at every edge with more to scroll to" },
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
      "group/bar flex touch-none p-2 opacity-0 select-none transition-control data-hovering:opacity-100 data-scrolling:opacity-100",
      orientation === "vertical" ? "w-10" : "h-10 flex-col",
    )}
  >
    <BaseScrollArea.Thumb className="flex-1 rounded-pill bg-line-strong transition-control group-data-hovering/bar:bg-fg-tertiary group-data-scrolling/bar:bg-fg-tertiary forced-line" />
  </BaseScrollArea.Scrollbar>
);

export const ScrollArea = forwardRef<HTMLDivElement, ScrollAreaProps>(function ScrollArea(
  { children, orientation = "vertical", className, viewportClassName, "aria-label": label },
  ref,
) {
  return (
    <CSPProvider disableStyleElements>
      <BaseScrollArea.Root ref={ref} className={cn("relative min-h-0 overflow-hidden", className)}>
        <BaseScrollArea.Viewport
          className={cn("peer/viewport size-full outline-none", KAESO_SCROLL_AREA_VARIANTS.orientation[orientation].classes, viewportClassName)}
          {...(label ? { role: "region", "aria-label": label } : {})}
        >
          {children}
        </BaseScrollArea.Viewport>
        <span
          aria-hidden
          style={{ borderRadius: "inherit" }}
          className="pointer-events-none absolute inset-0 peer-focus-visible/viewport:outline-2 peer-focus-visible/viewport:-outline-offset-2 peer-focus-visible/viewport:outline-focus peer-focus-visible/viewport:outline-solid"
        />
        {orientation !== "horizontal" && <Bar orientation="vertical" />}
        {orientation !== "vertical" && <Bar orientation="horizontal" />}
        {orientation === "both" && <BaseScrollArea.Corner />}
      </BaseScrollArea.Root>
    </CSPProvider>
  );
});
ScrollArea.displayName = "ScrollArea";
