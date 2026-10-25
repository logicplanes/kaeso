import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";

export const KAESO_GRANT_SUMMARY_VARIANTS = {} as const;
export const KAESO_GRANT_SUMMARY_DEFAULT_VARIANTS = {} as const;

export interface GrantSummaryProps {
  items: readonly ReactNode[];
  footer?: ReactNode | undefined;
  action?: ReactNode | undefined;
  "aria-label": string;
  className?: string | undefined;
}

/** What an agent or a key may do, one short line per item after a check mark, on a group surface, with an optional note and action below. */
export const GrantSummary = forwardRef<HTMLDivElement, GrantSummaryProps>(function GrantSummary(
  { items, footer, action, "aria-label": ariaLabel, className },
  ref,
) {
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
      {(footer || action) && (
        <div className="flex items-center justify-between gap-12 pt-12 hairline-t">
          <p className="min-w-0 text-small text-fg-tertiary text-pretty">{footer}</p>
          {action && <div className="shrink-0">{action}</div>}
        </div>
      )}
    </div>
  );
});
GrantSummary.displayName = "GrantSummary";
