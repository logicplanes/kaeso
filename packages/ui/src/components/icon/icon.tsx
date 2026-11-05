import { forwardRef, useState, type SVGAttributes } from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { cn } from "../../utils/cn.js";

export type IconData = IconSvgElement;

export const KAESO_ICON_VARIANTS = {
  size: {
    xs: { classes: "size-14", description: "14px: chips, xs controls, inline hints" },
    sm: { classes: "size-16", description: "16px: the default in controls and menus" },
    md: { classes: "size-18", description: "18px: sidebar items, lg controls" },
    lg: { classes: "size-20", description: "20px: xl controls, empty states" },
    xl: { classes: "size-24", description: "24px: rare, large moments" },
  },
} as const;
export const KAESO_ICON_DEFAULT_VARIANTS = { size: "sm" } as const;
export type IconSize = keyof typeof KAESO_ICON_VARIANTS.size;

const PX: Record<IconSize, number> = { xs: 14, sm: 16, md: 18, lg: 20, xl: 24 };
const STROKE = 1.5;

export interface IconProps extends Omit<SVGAttributes<SVGSVGElement>, "children" | "strokeWidth" | "color"> {
  icon: IconData;
  size?: IconSize | undefined;
  label?: string | undefined;
}

interface Swap {
  icon: IconData;
  last: IconData | null;
  turn: number;
}

export const Icon = forwardRef<SVGSVGElement, IconProps>(function Icon({ icon, size = KAESO_ICON_DEFAULT_VARIANTS.size, label, className, ...props }, ref) {
  const [swap, setSwap] = useState<Swap>({ icon, last: null, turn: 0 });
  if (swap.icon !== icon) setSwap({ icon, last: swap.icon, turn: swap.turn + 1 });
  const named = label ? { role: "img", "aria-label": label, "aria-hidden": undefined } : { "aria-hidden": props["aria-hidden"] ?? true, focusable: false };
  const draw = (data: IconData) => ({ icon: data, size: PX[size], strokeWidth: STROKE, absoluteStrokeWidth: true, color: "currentColor" });
  if (!swap.last) {
    return <HugeiconsIcon ref={ref} {...draw(icon)} {...props} className={cn("shrink-0", KAESO_ICON_VARIANTS.size[size].classes, className)} {...named} />;
  }
  return (
    <span
      className={cn("relative inline-flex shrink-0", KAESO_ICON_VARIANTS.size[size].classes, className)}
      {...(label ? { role: "img", "aria-label": label } : { "aria-hidden": true })}
    >
      <HugeiconsIcon key={`out-${swap.turn}`} {...draw(swap.last)} aria-hidden focusable={false} className="absolute inset-0 size-full icon-swap-out" />
      <HugeiconsIcon key={`in-${swap.turn}`} ref={ref} {...draw(icon)} {...props} aria-hidden focusable={false} className="size-full icon-swap-in" />
    </span>
  );
});
Icon.displayName = "Icon";
