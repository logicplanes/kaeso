import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { Icon, type IconData } from "../icon/icon.js";
import { useAppFrame } from "../app-frame/app-frame.js";
import { Text } from "../text/text.js";

export const KAESO_PAGE_HEADER_VARIANTS = {
  size: {
    md: {
      classes: "min-h-52 flex-wrap gap-x-12 gap-y-8 px-24 py-10 sm:h-52 sm:flex-nowrap sm:py-0",
      description:
        "52px, the title alone at 17/600; a hairline at its foot only while the page scrolls under it (default); on a phone the actions sit beside a title that fits and under one that doesn't, wrap rather than run off the edge, and the tabs take a row of their own",
    },
  },
} as const;
export const KAESO_PAGE_HEADER_DEFAULT_VARIANTS = { size: "md" } as const;

export interface PageHeaderProps {
  title?: ReactNode | undefined;
  icon?: IconData | undefined;
  tabs?: ReactNode | undefined;
  actions?: ReactNode | undefined;
  start?: ReactNode | undefined;
  className?: string | undefined;
}

export const PageHeader = forwardRef<HTMLElement, PageHeaderProps>(function PageHeader({ title, icon, tabs, actions, start, className }, ref) {
  const Heading = useAppFrame().embedded ? "h2" : "h1";
  return (
    <header ref={ref} className={cn("flex shrink-0 items-center", KAESO_PAGE_HEADER_VARIANTS.size.md.classes, className)}>
      {(start || title) && (
        // From sm up this box steps aside (contents), so the start keeps its width and only the heading gives way.
        <div className="flex min-w-0 flex-auto shrink-4 items-center gap-x-12 max-sm:min-w-96 sm:contents">
          {start}
          {title && (
            // flex-initial alone would shrink the title no faster than the tabs, cutting the last tab while letters remain.
            <Heading className="flex min-w-0 flex-auto items-center gap-8 text-title-sm font-semibold text-fg sm:flex-initial sm:shrink-1000">
              {icon && <Icon icon={icon} size="sm" className="shrink-0 text-fg-secondary" />}
              <Text truncate variant="title-sm" tone="inherit">
                {title}
              </Text>
            </Heading>
          )}
        </div>
      )}
      {tabs && <div className="order-last min-w-0 basis-full sm:order-none sm:ml-8 sm:basis-auto">{tabs}</div>}
      <div className="ml-auto flex max-w-full shrink-0 flex-wrap items-center justify-end gap-8">{actions}</div>
    </header>
  );
});
PageHeader.displayName = "PageHeader";
