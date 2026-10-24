import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
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
  className?: string | undefined;
}

export const Figure = forwardRef<HTMLElement, FigureProps>(function Figure({ children, label, caption, surface = "well", className }, ref) {
  return (
    <figure ref={ref} className={cn("flex min-w-0 flex-col gap-12", className)}>
      <ScrollArea orientation="horizontal" aria-label={label} className={KAESO_FIGURE_VARIANTS.surface[surface].classes}>
        <div className="mx-auto flex w-fit min-w-full justify-center p-16 sm:p-24">{children}</div>
      </ScrollArea>
      {caption && <figcaption className="text-center text-small text-pretty text-fg-secondary">{caption}</figcaption>}
    </figure>
  );
});
Figure.displayName = "Figure";
