import { createElement, forwardRef, useState, type Ref, type SVGAttributes } from "react";
import { cn } from "../../utils/cn.js";

/** An icon's shapes on Octicons' 16px grid, filled in the text's color: each element's tag and its attributes. */
export type IconData = readonly (readonly [tag: string, attrs: Readonly<Record<string, string | number | undefined>>])[];

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

type GlyphProps = Omit<SVGAttributes<SVGSVGElement>, "children" | "strokeWidth" | "color"> & { key?: number | undefined; ref?: Ref<SVGSVGElement> | undefined };

const glyph = (icon: IconData, px: number, props: GlyphProps) =>
  createElement(
    "svg",
    {
      width: px,
      height: px,
      viewBox: "0 0 16 16",
      fill: "currentColor",
      "aria-hidden": true,
      focusable: false,
      ...props,
    },
    ...icon.map(([tag, attrs]) => createElement(tag, attrs)),
  );

export const Icon = forwardRef<SVGSVGElement, IconProps>(function Icon({ icon, size = KAESO_ICON_DEFAULT_VARIANTS.size, label, className, ...props }, ref) {
  const [swap, setSwap] = useState<Swap>({ icon, last: null, turn: 0 });
  if (swap.icon !== icon) setSwap({ icon, last: swap.icon, turn: swap.turn + 1 });
  const named = label ? { role: "img", "aria-label": label } : { "aria-hidden": true };
  const px = PX[size];
  const box = cn("shrink-0", KAESO_ICON_VARIANTS.size[size].classes, className);
  if (!swap.last) return glyph(icon, px, { ref, ...props, className: box, ...(label && { ...named, "aria-hidden": undefined }) });
  return (
    <span className={cn("relative inline-flex", box)} {...named}>
      {glyph(swap.last, px, { key: -swap.turn, className: "absolute inset-0 size-full icon-swap-out" })}
      {glyph(icon, px, { ref, ...props, key: swap.turn, "aria-hidden": true, className: "size-full icon-swap-in" })}
    </span>
  );
});
Icon.displayName = "Icon";
