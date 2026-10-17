import { forwardRef, type SVGAttributes } from "react";
import { cn } from "../../utils/cn.js";
import { wordmark } from "./paths.js";

const WORDMARK_TIGHT = "72 69.3 881.9 177.7";

export interface NavWordmarkProps extends Omit<SVGAttributes<SVGSVGElement>, "children"> {
  title?: string | undefined;
  decorative?: boolean | undefined;
}

export const NavWordmark = forwardRef<SVGSVGElement, NavWordmarkProps>(function NavWordmark({ title = "Kaeso", decorative, className, style, ...props }, ref) {
  return (
    <svg
      ref={ref}
      viewBox={WORDMARK_TIGHT}
      fill="currentColor"
      xmlns="http://www.w3.org/2000/svg"
      className={cn("block shrink-0", className)}
      style={{ width: 60, height: 12, ...style }}
      {...(decorative ? { "aria-hidden": true, focusable: false } : { role: "img", "aria-label": title })}
      {...props}
    >
      {wordmark.paths.map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  );
});
NavWordmark.displayName = "Logo.Wordmark";
