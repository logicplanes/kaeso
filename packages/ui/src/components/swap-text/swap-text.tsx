import { forwardRef, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn";

export const KAESO_SWAP_TEXT_VARIANTS = {
  align: {
    center: { classes: "justify-items-center", description: "Centered in the reserved width (default)" },
    start: { classes: "justify-items-start", description: "Aligned to the start" },
  },
} as const;
export const KAESO_SWAP_TEXT_DEFAULT_VARIANTS = { align: "center" } as const;

export interface SwapTextProps extends Omit<HTMLAttributes<HTMLSpanElement>, "children"> {
  children: ReactNode;
  reserve: ReactNode[];
  align?: keyof typeof KAESO_SWAP_TEXT_VARIANTS.align | undefined;
}

export const SwapText = forwardRef<HTMLSpanElement, SwapTextProps>(function SwapText({ children, reserve, align = "center", className, ...props }, ref) {
  return (
    <span ref={ref} className={cn("inline-grid", KAESO_SWAP_TEXT_VARIANTS.align[align].classes, className)} {...props}>
      <span className="col-start-1 row-start-1">{children}</span>
      {reserve.map((r, i) => (
        <span key={i} aria-hidden className="invisible col-start-1 row-start-1">
          {r}
        </span>
      ))}
    </span>
  );
});
SwapText.displayName = "SwapText";
