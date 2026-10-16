import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge, validators } from "tailwind-merge";
import { lengths, radius, shadow as shadowTokens, type as typeScale } from "@logicplanes/kaeso-tokens";

const colors: string[] = [
  "panel",
  "chrome",
  "raised",
  "tooltip",
  "toast",
  "toast-fg",
  "toast-fg-secondary",
  "toast-action",
  "toast-action-hover",
  "toast-danger",
  "fill-1",
  "fill-2",
  "fill-3",
  "item-hover",
  "item-pressed",
  "line-subtle",
  "line",
  "line-strong",
  "panel-ring",
  "fg",
  "fg-secondary",
  "fg-tertiary",
  "fg-disabled",
  "ink",
  "on-ink",
  "focus",
  "overlay",
  "danger-fill",
  "on-danger",
  "success",
  "success-text",
  "success-tint",
  "warning",
  "warning-text",
  "warning-tint",
  "danger",
  "danger-text",
  "danger-tint",
  "info",
  "info-text",
  "info-tint",
  "viz-1",
  "viz-thin-1",
  "viz-2",
  "viz-thin-2",
  "viz-3",
  "viz-thin-3",
  "viz-4",
  "viz-thin-4",
  "viz-5",
  "viz-thin-5",
  "viz-6",
  "viz-thin-6",
  "viz-7",
  "viz-thin-7",
  "viz-8",
  "viz-thin-8",
];

function createMerge() {
  const shadowNames: ReadonlySet<string> = new Set(Object.keys(shadowTokens));
  const shadowColors = colors.filter((c) => !shadowNames.has(c));
  return extendTailwindMerge<"material" | "focus" | "motion">({
    override: {
      classGroups: {
        "shadow-color": [{ shadow: [...shadowColors, validators.isArbitraryVariable, validators.isArbitraryValue] }],
      },
    },
    extend: {
      theme: {
        color: colors,
        text: Object.keys(typeScale),
        spacing: lengths.map(String),
        radius: Object.keys(radius),
        shadow: Object.keys(shadowTokens),
        "font-weight": ["regular", "medium", "semibold"],
        ease: ["standard", "overlay", "exit", "pulse"],
      },
      classGroups: {
        material: [
          "material-primary",
          "material-secondary",
          "material-ghost",
          "material-soft",
          "material-danger",
          "material-danger-secondary",
          "material-field",
        ],
        focus: ["focus-ring", "focus-ring-inset"],
        motion: ["popup-motion", "dialog-motion", "sheet-motion", "backdrop-motion", "indicator-motion"],
        "min-w": [{ "min-w": ["anchor"] }],
        w: [{ w: ["anchor"] }],
      },
    },
  });
}

let merge: ReturnType<typeof createMerge> | undefined;

export function cn(...inputs: ClassValue[]): string {
  merge ??= createMerge();
  return merge(clsx(inputs));
}
