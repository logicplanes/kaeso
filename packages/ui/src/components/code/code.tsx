import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "../../utils/cn";

export const KAESO_CODE_VARIANTS = {
  shape: {
    auto: { classes: "", description: "Pill for a single word, radius 6 for anything longer (default)" },
    pill: { classes: "rounded-pill", description: "Always a pill" },
    rounded: { classes: "rounded-code", description: "Always radius 6" },
  },
} as const;
export const KAESO_CODE_DEFAULT_VARIANTS = { shape: "auto" } as const;

export interface CodeProps extends HTMLAttributes<HTMLElement> {
  shape?: keyof typeof KAESO_CODE_VARIANTS.shape | undefined;
}

export const Code = forwardRef<HTMLElement, CodeProps>(function Code({ shape = "auto", className, children, ...props }, ref) {
  const oneWord = typeof children === "string" && !/\s/.test(children);
  return (
    <code
      ref={ref}
      className={cn(
        "mono inline-block bg-fill-1 px-6 py-2 align-baseline text-small leading-none text-fg",
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
