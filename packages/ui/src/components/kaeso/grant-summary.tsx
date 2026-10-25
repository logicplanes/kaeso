import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";

export interface GrantSummaryProps {
  items: readonly ReactNode[];
  footer?: ReactNode | undefined;
  "aria-label": string;
  className?: string | undefined;
}

/** What an agent or a key may do, one short line per item after a check mark, on a group surface with an optional note below. */
export const GrantSummary = forwardRef<HTMLDivElement, GrantSummaryProps>(function GrantSummary({ items, footer, "aria-label": ariaLabel, className }, ref) {
  return (
    <div ref={ref} className={cn("flex flex-col gap-16 rounded-group bg-raised p-16 shadow-xs", className)}>
      <ul aria-label={ariaLabel} className="flex flex-col gap-10">
        {items.map((item, i) => (
          <li key={i} className="flex items-start gap-10 text-body text-fg">
            <Icon icon={icons.check} size="sm" className="mt-2 text-success-text" />
            <span className="min-w-0 text-pretty">{item}</span>
          </li>
        ))}
      </ul>
      {footer && <p className="text-small text-fg-tertiary text-pretty">{footer}</p>}
    </div>
  );
});
GrantSummary.displayName = "GrantSummary";
