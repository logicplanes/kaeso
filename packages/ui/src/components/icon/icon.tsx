import { forwardRef, type SVGAttributes } from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { cn } from "../../utils/cn";

export type IconData = IconSvgElement;

export const KAESO_ICON_VARIANTS = {
  size: {
    xs: { classes: "size-14", description: "14px, 1.75 stroke: chips, xs controls, inline hints" },
    sm: { classes: "size-16", description: "16px, 1.5 stroke: the default in controls and menus" },
    md: { classes: "size-18", description: "18px, 1.5 stroke: sidebar items, lg controls" },
    lg: { classes: "size-20", description: "20px, 1.5 stroke: xl controls, empty states" },
    xl: { classes: "size-24", description: "24px, 1.5 stroke: rare, large moments" },
  },
} as const;
export const KAESO_ICON_DEFAULT_VARIANTS = { size: "sm" } as const;
export type IconSize = keyof typeof KAESO_ICON_VARIANTS.size;

const PX: Record<IconSize, number> = { xs: 14, sm: 16, md: 18, lg: 20, xl: 24 };
const STROKE: Record<IconSize, number> = { xs: 1.75, sm: 1.5, md: 1.5, lg: 1.5, xl: 1.5 };

export interface IconProps extends Omit<SVGAttributes<SVGSVGElement>, "children" | "strokeWidth" | "color"> {
  icon: IconData;
  size?: IconSize | undefined;
  label?: string | undefined;
}

export const Icon = forwardRef<SVGSVGElement, IconProps>(function Icon({ icon, size = KAESO_ICON_DEFAULT_VARIANTS.size, label, className, ...props }, ref) {
  return (
    <HugeiconsIcon
      ref={ref}
      icon={icon}
      size={PX[size]}
      strokeWidth={STROKE[size]}
      color="currentColor"
      className={cn("shrink-0", KAESO_ICON_VARIANTS.size[size].classes, className)}
      {...(label ? { role: "img", "aria-label": label } : { "aria-hidden": true, focusable: false })}
      {...props}
    />
  );
});
Icon.displayName = "Icon";
