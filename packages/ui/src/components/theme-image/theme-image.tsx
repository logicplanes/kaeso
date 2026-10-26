import { forwardRef } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_THEME_IMAGE_VARIANTS = {
  theme: {
    light: { classes: "theme-light-only", description: "Shown in the light theme and in print" },
    dark: { classes: "theme-dark-only", description: "Shown in the dark theme" },
  },
} as const;
export const KAESO_THEME_IMAGE_DEFAULT_VARIANTS = {} as const;

export interface ThemeImageProps {
  light: string;
  dark: string;
  alt: string;
  width?: number | undefined;
  height?: number | undefined;
  className?: string | undefined;
  "aria-describedby"?: string | undefined;
}

export const ThemeImage = forwardRef<HTMLSpanElement, ThemeImageProps>(function ThemeImage(
  { light, dark, alt, width, height, className, "aria-describedby": describedBy },
  ref,
) {
  const shared = {
    alt,
    loading: "lazy" as const,
    decoding: "async" as const,
    ...(width && height ? { width, height, style: { minWidth: `calc(${width}px * var(--k-figure-floor, 0))` } } : {}),
    ...(describedBy ? { "aria-describedby": describedBy } : {}),
  };
  return (
    <span ref={ref} className={cn("block max-w-full", className)}>
      <img src={light} {...shared} className={cn("h-auto max-w-full", KAESO_THEME_IMAGE_VARIANTS.theme.light.classes)} />
      <img src={dark} {...shared} className={cn("h-auto max-w-full", KAESO_THEME_IMAGE_VARIANTS.theme.dark.classes)} />
    </span>
  );
});
ThemeImage.displayName = "ThemeImage";
