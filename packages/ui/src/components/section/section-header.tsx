import { forwardRef, type ReactNode } from "react";

export const SectionHeader = forwardRef<HTMLElement, { title: ReactNode; description: ReactNode; actions: ReactNode }>(function SectionHeader(
  { title, description, actions },
  ref,
) {
  return (
    <header ref={ref} className="mb-16 flex flex-wrap items-end justify-between gap-x-16 gap-y-12">
      <div className="flex min-w-0 flex-col gap-2">
        {title && <h2 className="text-title-sm font-semibold text-fg">{title}</h2>}
        {description && <p className="max-w-prose text-small text-fg-secondary">{description}</p>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-8">{actions}</div>}
    </header>
  );
});
SectionHeader.displayName = "SectionHeader";
