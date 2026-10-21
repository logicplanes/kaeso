import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_SKELETON_VARIANTS = {
  shape: {
    block: { classes: "rounded-well", description: "Rounded blocks for charts and wells (default)" },
    line: { classes: "h-12 rounded-pill", description: "A pill line for text" },
    circle: { classes: "rounded-pill", description: "Avatars and icons" },
    control: { classes: "h-36 rounded-pill", description: "The shape of a 36px control" },
    "control-sm": { classes: "h-32 rounded-pill", description: "The shape of a 32px control, such as a small button in a page header" },
  },
} as const;
export const KAESO_SKELETON_DEFAULT_VARIANTS = { shape: "block" } as const;

export interface SkeletonProps extends HTMLAttributes<HTMLDivElement> {
  shape?: keyof typeof KAESO_SKELETON_VARIANTS.shape | undefined;
}

export const Skeleton = forwardRef<HTMLDivElement, SkeletonProps>(function Skeleton({ shape = "block", className, ...props }, ref) {
  return <div ref={ref} aria-hidden className={cn("bg-fill-2 animate-pulse-quiet", KAESO_SKELETON_VARIANTS.shape[shape].classes, className)} {...props} />;
});
Skeleton.displayName = "Skeleton";

export const SkeletonLine = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement> & { lines?: number | undefined }>(function SkeletonLine(
  { lines = 1, className, ...props },
  ref,
) {
  return (
    <div ref={ref} aria-hidden className={cn("flex flex-col gap-8", className)} {...props}>
      {Array.from({ length: lines }, (_, i) => (
        <Skeleton key={i} shape="line" className={i === lines - 1 && lines > 1 ? "w-3/5" : "w-full"} />
      ))}
    </div>
  );
});
SkeletonLine.displayName = "SkeletonLine";
