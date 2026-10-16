import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { Icon, type IconData } from "../icon/icon";
import { useAppFrame } from "../app-frame/app-frame";

export const KAESO_PAGE_HEADER_VARIANTS = {
  size: {
    md: {
      classes: "min-h-52 flex-wrap gap-x-12 gap-y-8 px-24 py-10 sm:h-52 sm:flex-nowrap sm:py-0",
      description: "52px, no bottom line (default); on a phone the tabs take a second row",
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
      {start}
      {title && (
        <Heading className="flex min-w-0 items-center gap-8 text-body-lg font-semibold text-fg">
          {icon && <Icon icon={icon} size="sm" className="text-fg-secondary" />}
          <span className="truncate">{title}</span>
        </Heading>
      )}
      {tabs && <div className="order-last -ml-12 min-w-0 basis-full sm:order-none sm:ml-8 sm:basis-auto">{tabs}</div>}
      <div className="ml-auto flex shrink-0 items-center gap-8">{actions}</div>
    </header>
  );
});
PageHeader.displayName = "PageHeader";
