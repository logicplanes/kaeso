import type { IconSize } from "../icon/icon.js";

export const KAESO_INPUT_GROUP_VARIANTS = {
  size: {
    sm: { classes: "h-32 px-12 gap-6 text-body", description: "32px: toolbars and compact forms" },
    md: { classes: "h-36 px-16 gap-8 text-body", description: "36px: the default" },
    lg: { classes: "h-40 px-18 gap-8 text-body", description: "40px" },
    xl: { classes: "h-48 px-22 gap-8 text-body-lg", description: "48px: sign-in and setup forms" },
  },
} as const;
export type InputSize = keyof typeof KAESO_INPUT_GROUP_VARIANTS.size;

export const LEADING: Record<InputSize, string> = { sm: "pl-10", md: "pl-14", lg: "pl-16", xl: "pl-20" };
export const INPUT_ICON_SIZE: Record<InputSize, IconSize> = { sm: "sm", md: "sm", lg: "md", xl: "lg" };
