import { forwardRef, useEffect, useRef, useState, type SVGAttributes } from "react";
import { cn } from "../../utils/cn";

export const KAESO_SPINNER_VARIANTS = {
  size: {
    xs: { classes: "size-14", description: "14px: xs controls" },
    sm: { classes: "size-16", description: "16px: buttons, toasts, labels (default)" },
    md: { classes: "size-20", description: "20px: xl buttons" },
    lg: { classes: "size-24", description: "24px: a region loading" },
    xl: { classes: "size-32", description: "32px: a page loading" },
  },
} as const;
export const KAESO_SPINNER_DEFAULT_VARIANTS = { size: "sm" } as const;

export type SpinnerSize = keyof typeof KAESO_SPINNER_VARIANTS.size;

export interface SpinnerProps extends Omit<SVGAttributes<SVGSVGElement>, "children"> {
  size?: SpinnerSize | undefined;
  label?: string | undefined;
}

export const Spinner = forwardRef<SVGSVGElement, SpinnerProps>(function Spinner({ size = "sm", label, className, ...props }, ref) {
  return (
    <svg
      ref={ref}
      viewBox="0 0 16 16"
      className={cn("k-spinner shrink-0", KAESO_SPINNER_VARIANTS.size[size].classes, className)}
      {...(label ? { role: "status", "aria-label": label } : { "aria-hidden": true })}
      {...props}
    >
      <path className="k-spinner-route" d="M9.931 2.056A6.25 6.25 0 0 1 14.113 9.299" />
      <path className="k-spinner-route" d="M9.931 2.056A6.25 6.25 0 0 1 14.113 9.299" />
      <path className="k-spinner-route" d="M9.931 2.056A6.25 6.25 0 0 1 14.113 9.299" />
    </svg>
  );
});
Spinner.displayName = "Spinner";

export function useDelayedLoading(loading: boolean, delay = 300, minDuration = 400): boolean {
  const [shown, setShown] = useState(loading && !delay);
  const until = useRef(0);
  useEffect(() => {
    if (loading === shown) return;
    const t = setTimeout(
      () => {
        until.current = Date.now() + minDuration;
        setShown(loading);
      },
      loading ? delay : until.current - Date.now(),
    );
    return () => clearTimeout(t);
  }, [loading, shown, delay, minDuration]);
  return shown;
}
