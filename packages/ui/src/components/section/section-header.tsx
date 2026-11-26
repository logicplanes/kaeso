import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";

export const SectionHeader = forwardRef<HTMLElement, { title: ReactNode; description: ReactNode; actions: ReactNode; rule?: boolean | undefined }>(
  function SectionHeader({ title, description, actions, rule = true }, ref) {
    return (
      <header
        ref={ref}
        className={cn("flex flex-wrap justify-between gap-x-16 gap-y-12 pb-12", rule ? "mb-16 rule-b" : "mb-4", description ? "items-end" : "items-center")}
      >
        <div className="flex min-w-0 flex-col gap-2 sm:grow sm:basis-240">
          {title && <h2 className="text-title-sm font-semibold text-balance text-fg">{title}</h2>}
          {description && <p className="text-small text-pretty text-fg-secondary">{description}</p>}
        </div>
        {actions && <div className="flex max-w-full shrink-0 flex-wrap items-center gap-8 *:max-w-full *:min-w-0">{actions}</div>}
      </header>
    );
  },
);
SectionHeader.displayName = "SectionHeader";
