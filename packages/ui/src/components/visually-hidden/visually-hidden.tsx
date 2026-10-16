import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "../../utils/cn.js";

export const KAESO_VISUALLY_HIDDEN_VARIANTS = {} as const;
export const KAESO_VISUALLY_HIDDEN_DEFAULT_VARIANTS = {} as const;

export const VisuallyHidden = forwardRef<HTMLSpanElement, HTMLAttributes<HTMLSpanElement>>(function VisuallyHidden({ className, ...props }, ref) {
  return <span ref={ref} className={cn("sr-only", className)} {...props} />;
});
VisuallyHidden.displayName = "VisuallyHidden";
