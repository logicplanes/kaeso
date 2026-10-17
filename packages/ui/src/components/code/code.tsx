import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_CODE_VARIANTS = {
  shape: {
    auto: { classes: "", description: "Pill for a single word, radius 6 for anything longer (default)" },
    pill: { classes: "rounded-pill", description: "Always a pill" },
    rounded: { classes: "rounded-code", description: "Always radius 6" },
  },
  flow: {
    box: { classes: "inline-block", description: "One box: it moves to the next line whole and wraps inside itself (default)" },
    text: {
      classes: "inline box-decoration-clone",
      description: "Breaks with the sentence around it, each line keeping its fill and corners; for long code in running text",
    },
    cell: {
      classes: "inline",
      description: "Breaks with the text around it and ends square at a break, so no padding hangs past a narrow table cell; for long code in a table",
    },
  },
} as const;
export const KAESO_CODE_DEFAULT_VARIANTS = { shape: "auto", flow: "box" } as const;

export interface CodeProps extends HTMLAttributes<HTMLElement> {
  shape?: keyof typeof KAESO_CODE_VARIANTS.shape | undefined;
  flow?: keyof typeof KAESO_CODE_VARIANTS.flow | undefined;
}

export const Code = forwardRef<HTMLElement, CodeProps>(function Code({ shape = "auto", flow = "box", className, children, ...props }, ref) {
  const oneWord = typeof children === "string" && !/\s/.test(children);
  return (
    <code
      ref={ref}
      className={cn(
        "mono bg-fill-1 px-4 py-2 align-baseline text-small leading-none text-fg",
        KAESO_CODE_VARIANTS.flow[flow].classes,
        shape === "auto" ? (oneWord ? "rounded-pill" : "rounded-code") : KAESO_CODE_VARIANTS.shape[shape].classes,
        className,
      )}
      {...props}
    >
      {children}
    </code>
  );
});
Code.displayName = "Code";
