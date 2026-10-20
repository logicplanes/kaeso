import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { NavWordmark } from "../logo/nav-wordmark.js";

export const KAESO_SECTION_VARIANTS = {
  spacing: {
    default: { classes: "mt-48 first:mt-0", description: "48px between sections (default)" },
    none: { classes: "", description: "No outer spacing" },
  },
} as const;
export const KAESO_SECTION_DEFAULT_VARIANTS = { spacing: "default" } as const;

export interface SectionProps {
  title?: ReactNode | undefined;
  description?: ReactNode | undefined;
  actions?: ReactNode | undefined;
  spacing?: keyof typeof KAESO_SECTION_VARIANTS.spacing | undefined;
  children: ReactNode;
  className?: string | undefined;
}

export const Section = forwardRef<HTMLElement, SectionProps>(function Section({ title, description, actions, spacing = "default", children, className }, ref) {
  return (
    <section ref={ref} className={cn(KAESO_SECTION_VARIANTS.spacing[spacing].classes, className)}>
      {(title || actions) && (
        <header className="mb-16 flex flex-wrap items-end justify-between gap-x-16 gap-y-12">
          <div className="flex min-w-0 flex-col gap-2">
            {title && <h2 className="text-title-sm font-semibold text-fg">{title}</h2>}
            {description && <p className="max-w-prose text-small text-fg-secondary">{description}</p>}
          </div>
          {actions && <div className="flex shrink-0 items-center gap-8">{actions}</div>}
        </header>
      )}
      {children}
    </section>
  );
});
Section.displayName = "Section";

export const SplitView = forwardRef<HTMLDivElement, { children: ReactNode; aside: ReactNode; className?: string | undefined }>(function SplitView(
  { children, aside, className },
  ref,
) {
  return (
    <div ref={ref} className={cn("grid grid-cols-1 lg:grid-cols-aside", className)}>
      <div className="min-w-0 lg:pr-32">{children}</div>
      <aside className="mt-32 min-w-0 lg:mt-0 lg:pl-32 lg:hairline-l">
        <div className="flex flex-col gap-32 lg:sticky lg:top-16">{aside}</div>
      </aside>
    </div>
  );
});
SplitView.displayName = "SplitView";

export const AuthLayout = forwardRef<
  HTMLDivElement,
  { title: ReactNode; description?: ReactNode | undefined; children: ReactNode; footer?: ReactNode | undefined; className?: string | undefined }
>(function AuthLayout({ title, description, children, footer, className }, ref) {
  return (
    <div ref={ref} className={cn("relative flex min-h-dvh flex-col items-center bg-panel px-16 py-64 text-fg", className)}>
      <div className="absolute top-0 left-0 mt-8 flex h-52 items-center px-16 sm:px-28">
        <NavWordmark />
      </div>
      <div className="flex w-full max-w-auth flex-1 flex-col justify-center">
        <h1 className="text-center text-title-lg font-semibold text-fg text-balance">{title}</h1>
        {description && <p className="mt-8 text-center text-body-lg font-regular text-fg-secondary text-pretty">{description}</p>}
        <div className="mt-24 flex flex-col gap-16">{children}</div>
      </div>
      {footer && <div className="mt-48 text-small text-fg-tertiary">{footer}</div>}
    </div>
  );
});
AuthLayout.displayName = "AuthLayout";
