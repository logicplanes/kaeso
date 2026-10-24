import { forwardRef, useLayoutEffect, useRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { observeResize } from "../../utils/observe-resize.js";
import { ScrollArea } from "../scroll-area/scroll-area.js";

export const KAESO_FIGURE_VARIANTS = {
  surface: {
    well: { classes: "rounded-well bg-fill-1", description: "A well like a code block's: radius 14, fill, no ring (default)" },
    plain: { classes: "", description: "No surface, for a figure that brings its own" },
  },
} as const;
export const KAESO_FIGURE_DEFAULT_VARIANTS = { surface: "well" } as const;

export interface FigureProps {
  children: ReactNode;
  label: string;
  caption?: ReactNode | undefined;
  surface?: keyof typeof KAESO_FIGURE_VARIANTS.surface | undefined;
  focus?: number | undefined;
  className?: string | undefined;
}

const TOUCHED = ["pointerdown", "wheel", "keydown"] as const;

function useFocused(focus: number) {
  const content = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const inner = content.current;
    const viewport = inner?.parentElement;
    if (!inner || !viewport) return;
    let touched = false;
    const touch = () => {
      touched = true;
    };
    const centre = () => {
      const room = viewport.scrollWidth - viewport.clientWidth;
      if (!touched) viewport.scrollLeft = Math.min(room, Math.max(0, focus * viewport.scrollWidth - viewport.clientWidth / 2));
    };
    centre();
    for (const name of TOUCHED) viewport.addEventListener(name, touch, { passive: true });
    const stop = observeResize([viewport, inner], centre);
    return () => {
      stop();
      for (const name of TOUCHED) viewport.removeEventListener(name, touch);
    };
  }, [focus]);
  return content;
}

export const Figure = forwardRef<HTMLElement, FigureProps>(function Figure({ children, label, caption, surface = "well", focus = 0.5, className }, ref) {
  const content = useFocused(focus);
  return (
    <figure ref={ref} className={cn("flex min-w-0 flex-col gap-12", className)}>
      <ScrollArea orientation="horizontal" aria-label={label} className={KAESO_FIGURE_VARIANTS.surface[surface].classes}>
        <div ref={content} className="mx-auto flex w-fit min-w-full justify-center p-16 sm:p-24">
          {children}
        </div>
      </ScrollArea>
      {caption && <figcaption className="text-center text-small text-pretty text-fg-secondary">{caption}</figcaption>}
    </figure>
  );
});
Figure.displayName = "Figure";
