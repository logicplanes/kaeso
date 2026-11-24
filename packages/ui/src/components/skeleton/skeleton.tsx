import { forwardRef, type HTMLAttributes, type Ref } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_SKELETON_VARIANTS = {
  shape: {
    block: { classes: "rounded-well", description: "A cut plate for charts and wells (default)" },
    line: { classes: "h-12 rounded-bar", description: "A bar for a line of text, cut like a plate" },
    word: {
      classes: "inline-block h-12 rounded-bar align-middle",
      description: "A bar inside a line of text, such as a title or a note still loading: it sits on the text's line, so the line keeps its height",
    },
    circle: { classes: "rounded-pill", description: "Avatars and icons" },
    control: { classes: "h-36 rounded-pill", description: "The shape of a 36px control" },
    trend: { classes: "h-48 rounded-well", description: "The plate of a 48px trend line, such as a dashboard cell's fluid Sparkline" },
  },
} as const;
export const KAESO_SKELETON_DEFAULT_VARIANTS = { shape: "block" } as const;

export interface SkeletonProps extends HTMLAttributes<HTMLDivElement> {
  shape?: keyof typeof KAESO_SKELETON_VARIANTS.shape | undefined;
  lines?: number | undefined;
}

const BASE = "bg-fill-2 animate-pulse-quiet forced-colors:outline forced-colors:-outline-offset-1";

export const Skeleton = forwardRef<HTMLDivElement, SkeletonProps>(function Skeleton({ shape = "block", lines, className, ...props }, ref) {
  if (lines !== undefined) {
    return (
      <div ref={ref} aria-hidden className={cn("flex flex-col gap-8", className)} {...props}>
        {Array.from({ length: lines }, (_, i) => (
          <div key={i} className={cn(BASE, KAESO_SKELETON_VARIANTS.shape.line.classes, i === lines - 1 && lines > 1 ? "w-3/5" : "w-full")} />
        ))}
      </div>
    );
  }
  const classes = cn(BASE, KAESO_SKELETON_VARIANTS.shape[shape].classes, className);
  // A word stands inside text, often a <p> or a heading, where a <div> isn't allowed.
  if (shape === "word") return <span ref={ref as Ref<HTMLSpanElement>} aria-hidden className={classes} {...props} />;
  return <div ref={ref} aria-hidden className={classes} {...props} />;
});
Skeleton.displayName = "Skeleton";
