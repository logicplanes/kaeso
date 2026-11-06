import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { Collapsible } from "../collapsible/collapsible.js";
import { Code } from "../code/code.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";

export const KAESO_GRANT_SUMMARY_VARIANTS = {} as const;
export const KAESO_GRANT_SUMMARY_DEFAULT_VARIANTS = {} as const;

export interface GrantSummarySection {
  id: string;
  title: ReactNode;
  hint?: ReactNode | undefined;
  items: readonly ReactNode[];
  defaultOpen?: boolean | undefined;
}

export interface GrantSummaryProps {
  items?: readonly ReactNode[] | undefined;
  sections?: readonly GrantSummarySection[] | undefined;
  footer?: ReactNode | undefined;
  action?: ReactNode | undefined;
  "aria-label": string;
  className?: string | undefined;
}

/** What an agent or a key may do, one short line per item after a check mark, on a group surface, with an optional note and action below. */
export const GrantSummary = forwardRef<HTMLDivElement, GrantSummaryProps>(function GrantSummary(
  { items = [], sections, footer, action, "aria-label": ariaLabel, className },
  ref,
) {
  const grouped = !!sections?.length;
  return (
    <div ref={ref} className={cn("flex flex-col gap-16 rounded-group bg-raised shadow-xs", grouped ? "px-8 pt-8 pb-12" : "p-20", className)}>
      {grouped ? (
        <ul aria-label={ariaLabel} className="flex flex-col">
          {sections.map((section) => (
            <li key={section.id}>
              <Collapsible title={section.title} hint={section.hint} {...(section.defaultOpen !== undefined ? { defaultOpen: section.defaultOpen } : {})}>
                <ul className="flex flex-wrap gap-6">
                  {section.items.map((item, i) => (
                    <li key={i}>
                      <Code>{item}</Code>
                    </li>
                  ))}
                </ul>
              </Collapsible>
            </li>
          ))}
        </ul>
      ) : (
        <ul aria-label={ariaLabel} className="flex flex-col gap-10">
          {items.map((item, i) => (
            <li key={i} className="flex items-start gap-10 text-body text-fg">
              <Icon icon={icons.check} size="sm" className="mt-2 text-success-text" />
              <span className="min-w-0 text-pretty">{item}</span>
            </li>
          ))}
        </ul>
      )}
      {(footer || action) && (
        <div className={cn("flex items-center justify-between gap-12 pt-12 hairline-t", grouped && "px-12")}>
          {footer && <p className="min-w-0 text-small text-fg-secondary text-pretty">{footer}</p>}
          {action && <div className="ml-auto shrink-0">{action}</div>}
        </div>
      )}
    </div>
  );
});
GrantSummary.displayName = "GrantSummary";
