import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_HEADING_VARIANTS = {
  size: {
    "title-sm": { classes: "text-title-sm font-semibold", description: "17/24: section, dialog and sheet titles" },
    title: { classes: "text-title font-semibold", description: "20/28: empty states, compact page titles" },
    "title-lg": { classes: "text-title-lg font-semibold", description: "28/34: settings page titles, auth screens" },
    display: { classes: "text-display font-semibold", description: "40/44: showcase headings, the setup screen" },
  },
} as const;
export const KAESO_HEADING_DEFAULT_VARIANTS = { size: "title-sm" } as const;

const DEFAULT_LEVEL: Record<keyof typeof KAESO_HEADING_VARIANTS.size, 1 | 2 | 3> = {
  display: 1,
  "title-lg": 1,
  title: 2,
  "title-sm": 2,
};

export interface HeadingProps extends HTMLAttributes<HTMLHeadingElement> {
  size?: keyof typeof KAESO_HEADING_VARIANTS.size | undefined;
  level?: 1 | 2 | 3 | 4 | 5 | 6 | undefined;
  balance?: boolean | undefined;
}

export const Heading = forwardRef<HTMLHeadingElement, HeadingProps>(function Heading(
  { size = KAESO_HEADING_DEFAULT_VARIANTS.size, level, balance = true, className, ...props },
  ref,
) {
  const Tag = `h${level ?? DEFAULT_LEVEL[size]}` as "h1";
  return <Tag ref={ref} className={cn("text-fg", KAESO_HEADING_VARIANTS.size[size].classes, balance && "text-balance", className)} {...props} />;
});
Heading.displayName = "Heading";
