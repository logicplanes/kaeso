import { forwardRef, type ElementType, type HTMLAttributes } from "react";
import { cn } from "../../utils/cn.js";

const GAP = {
  0: "gap-0",
  2: "gap-2",
  4: "gap-4",
  6: "gap-6",
  8: "gap-8",
  12: "gap-12",
  16: "gap-16",
  20: "gap-20",
  24: "gap-24",
  32: "gap-32",
  40: "gap-40",
  48: "gap-48",
  64: "gap-64",
} as const;
export type Gap = keyof typeof GAP;

export const KAESO_LAYOUT_VARIANTS = {
  align: {
    start: { classes: "items-start", description: "Align to the start" },
    center: { classes: "items-center", description: "Center on the cross axis" },
    end: { classes: "items-end", description: "Align to the end" },
    baseline: { classes: "items-baseline", description: "Align text baselines" },
    stretch: { classes: "items-stretch", description: "Stretch" },
  },
  justify: {
    start: { classes: "justify-start", description: "Pack to the start" },
    center: { classes: "justify-center", description: "Center" },
    end: { classes: "justify-end", description: "Pack to the end" },
    between: { classes: "justify-between", description: "Space between" },
  },
  columns: {
    1: { classes: "grid-cols-1", description: "One column" },
    2: { classes: "grid-cols-1 md:grid-cols-2", description: "Two columns from 768px" },
    3: { classes: "grid-cols-1 md:grid-cols-3", description: "Three columns from 768px" },
    4: { classes: "grid-cols-2 lg:grid-cols-4", description: "Two, then four from 1024px" },
    6: { classes: "grid-cols-3 lg:grid-cols-6", description: "Three, then six from 1024px" },
  },
} as const;
export const KAESO_LAYOUT_DEFAULT_VARIANTS = { align: "stretch", justify: "start", columns: 2 } as const;

interface BaseProps extends HTMLAttributes<HTMLElement> {
  gap?: Gap | undefined;
  align?: keyof typeof KAESO_LAYOUT_VARIANTS.align | undefined;
  justify?: keyof typeof KAESO_LAYOUT_VARIANTS.justify | undefined;
  as?: ElementType | undefined;
}

export const Stack = forwardRef<HTMLElement, BaseProps>(function Stack({ gap = 12, align, justify, as, className, ...props }, ref) {
  const C = (as ?? "div") as ElementType;
  return (
    <C
      ref={ref}
      className={cn(
        "flex flex-col",
        GAP[gap],
        align && KAESO_LAYOUT_VARIANTS.align[align].classes,
        justify && KAESO_LAYOUT_VARIANTS.justify[justify].classes,
        className,
      )}
      {...props}
    />
  );
});
Stack.displayName = "Stack";

export const Inline = forwardRef<HTMLElement, BaseProps & { wrap?: boolean | undefined }>(function Inline(
  { gap = 8, align = "center", justify, wrap = true, as, className, ...props },
  ref,
) {
  const C = (as ?? "div") as ElementType;
  return (
    <C
      ref={ref}
      className={cn(
        "flex",
        wrap && "flex-wrap",
        GAP[gap],
        KAESO_LAYOUT_VARIANTS.align[align].classes,
        justify && KAESO_LAYOUT_VARIANTS.justify[justify].classes,
        className,
      )}
      {...props}
    />
  );
});
Inline.displayName = "Inline";

export const Grid = forwardRef<HTMLElement, Omit<BaseProps, "justify"> & { columns?: keyof typeof KAESO_LAYOUT_VARIANTS.columns | undefined }>(function Grid(
  { gap = 16, columns = 2, align, as, className, ...props },
  ref,
) {
  const C = (as ?? "div") as ElementType;
  return (
    <C
      ref={ref}
      className={cn("grid", GAP[gap], KAESO_LAYOUT_VARIANTS.columns[columns].classes, align && KAESO_LAYOUT_VARIANTS.align[align].classes, className)}
      {...props}
    />
  );
});
Grid.displayName = "Grid";
