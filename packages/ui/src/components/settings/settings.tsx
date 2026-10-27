import type * as React from "react";
import { forwardRef, type MouseEvent, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { useLinkComponent } from "../../utils/link-provider.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon, type IconData } from "../icon/icon.js";
import { SectionHeader } from "../section/section-header.js";

export const KAESO_SETTINGS_VARIANTS = {
  row: {
    default: { classes: "min-h-64 py-12", description: "Title and description: at least 64px" },
    single: { classes: "min-h-56 py-10", description: "One line: 56px" },
  },
} as const;
export const KAESO_SETTINGS_DEFAULT_VARIANTS = { row: "default" } as const;

export interface SettingsSectionProps {
  title?: ReactNode | undefined;
  description?: ReactNode | undefined;
  actions?: ReactNode | undefined;
  children: ReactNode;
  className?: string | undefined;
}

export const KAESO_SETTINGS_SECTION_VARIANTS = {} as const;
export const KAESO_SETTINGS_SECTION_DEFAULT_VARIANTS = {} as const;

export const SettingsSection = forwardRef<HTMLElement, SettingsSectionProps>(function SettingsSection(
  { title, description, actions, children, className },
  ref,
) {
  return (
    <section ref={ref} className={cn("mt-48 first:mt-40", className)}>
      {(title || actions) && <SectionHeader title={title} description={description} actions={actions} />}
      {children}
    </section>
  );
});
SettingsSection.displayName = "SettingsSection";

export const SettingsGroup = forwardRef<HTMLDivElement, { children: ReactNode; className?: string | undefined; "aria-label"?: string | undefined }>(
  function SettingsGroup({ children, className, ...props }, ref) {
    return (
      <div ref={ref} role="group" className={cn("flex flex-col overflow-hidden rounded-group bg-raised shadow-xs", className)} {...props}>
        {children}
      </div>
    );
  },
);
SettingsGroup.displayName = "SettingsGroup";

export interface SettingsRowProps {
  title: ReactNode;
  description?: ReactNode | undefined;
  icon?: IconData | undefined;
  control?: ReactNode | undefined;
  value?: ReactNode | undefined;
  href?: string | undefined;
  onClick?: ((e: MouseEvent<HTMLElement>) => void) | undefined;
  id?: string | undefined;
  danger?: boolean | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

export const SettingsRow = forwardRef<HTMLDivElement, SettingsRowProps>(function SettingsRow(
  { title, description, icon, control, value, href, onClick, id, danger, className, "data-force": force },
  ref,
) {
  const LinkComp = useLinkComponent();
  const navigates = !!(href || onClick);
  const body = (
    <>
      {icon && (
        <span className="inline-flex size-20 shrink-0 items-center justify-center text-fg-secondary">
          <Icon icon={icon} size="md" />
        </span>
      )}
      <span className="flex min-w-0 flex-1 basis-140 flex-col">
        <span id={id ? `${id}-title` : undefined} className={cn("text-body-lg font-medium", danger ? "text-danger-text" : "text-fg")}>
          {title}
        </span>
        {description && <span className="text-small text-fg-secondary text-pretty">{description}</span>}
      </span>
      {control && (
        <span data-row-control="" className={cn("flex max-w-full shrink-0 items-center @max-settings-row:basis-full", icon && "@max-settings-row:pl-36")}>
          {control}
        </span>
      )}
      {(value !== undefined || navigates) && !control && (
        <span className="flex shrink-0 items-center gap-6 text-body text-fg-secondary">
          {value}
          {navigates && <Icon icon={icons.chevronRight} size="sm" className="text-fg-tertiary" />}
        </span>
      )}
    </>
  );
  const classes = cn(
    "@container relative flex w-full flex-wrap items-center gap-x-16 gap-y-8 px-20 text-left",
    description ? KAESO_SETTINGS_VARIANTS.row.default.classes : KAESO_SETTINGS_VARIANTS.row.single.classes,
    "not-first:before:absolute not-first:before:top-0 not-first:before:right-20 not-first:before:h-1 not-first:before:bg-line-subtle not-first:before:forced-line",
    icon ? "not-first:before:left-56" : "not-first:before:left-20",
    navigates && "cursor-pointer transition-control hover:bg-raised-hover active:bg-raised-pressed focus-ring-inset first:rounded-t-group last:rounded-b-group",
    className,
  );
  if (href) {
    return (
      <LinkComp ref={ref as React.Ref<HTMLAnchorElement>} href={href} className={classes} {...(force ? { "data-force": force } : {})}>
        {body}
      </LinkComp>
    );
  }
  if (onClick) {
    return (
      <button ref={ref as React.Ref<HTMLButtonElement>} type="button" onClick={onClick} className={classes} {...(force ? { "data-force": force } : {})}>
        {body}
      </button>
    );
  }
  return (
    <div ref={ref} className={classes} {...(force ? { "data-force": force } : {})}>
      {body}
    </div>
  );
});
SettingsRow.displayName = "SettingsRow";
