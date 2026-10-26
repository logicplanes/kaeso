import { forwardRef, type Ref, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { devWarn } from "../../utils/dev.js";
import { icons } from "../../icons/vocabulary.js";
import { IconButton } from "../icon-button/icon-button.js";
import { Popover } from "../popover/popover.js";
import { Segmented, type SegmentedOption } from "../segmented/segmented.js";
import { Select, type SelectOption } from "../select/select.js";
import { Switch } from "../switch/switch.js";
import { Tag } from "../tag/tag.js";
import type { IconData } from "../icon/icon.js";
import { Icon } from "../icon/icon.js";
import { useLocale, useUiText } from "../../i18n/context.js";
import { RaisedContent } from "../../utils/inset.js";

export const KAESO_DISPLAY_OPTIONS_VARIANTS = {
  trigger: {
    secondary: { classes: "", description: "A raised circle with a ring, like page-header buttons (default)" },
    ghost: { classes: "", description: "A ghost circle" },
  },
} as const;
export const KAESO_DISPLAY_OPTIONS_DEFAULT_VARIANTS = { trigger: "secondary" } as const;

export interface DisplayOptionsRow {
  label: string;
  icon?: IconData | undefined;
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
}
export interface DisplayOptionsToggle {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}

export interface DisplayOptionsProps {
  view?: { value: string; options: SegmentedOption[]; onChange: (v: string) => void } | undefined;
  rows?: DisplayOptionsRow[] | undefined;
  toggles?: DisplayOptionsToggle[] | undefined;
  properties?: { options: { value: string; label: string }[]; value: string[]; onChange: (v: string[]) => void; hidden?: string[] | undefined } | undefined;
  trigger?: keyof typeof KAESO_DISPLAY_OPTIONS_VARIANTS.trigger | undefined;
  inline?: boolean | undefined;
  footer?: ReactNode | undefined;
}

function focusFirst(panel: HTMLDivElement | null) {
  if (!panel) return;
  // Base UI focuses the popup box when it opens; the first control takes focus on the next frame, after that.
  const frame = requestAnimationFrame(() =>
    panel.querySelector<HTMLElement>('[role="radio"][tabindex="0"], button:not([disabled]), [tabindex="0"]')?.focus({ preventScroll: true }),
  );
  return () => cancelAnimationFrame(frame);
}

function Panel({ view, rows, toggles, properties, footer, inline }: DisplayOptionsProps) {
  const t = useUiText();
  const locale = useLocale();
  const squeezed = properties ? properties.options.filter((o) => properties.value.includes(o.value) && properties.hidden?.includes(o.value)) : [];
  return (
    <RaisedContent>
      <div ref={inline ? undefined : focusFirst} className="flex w-320 max-w-(--available-width) flex-col">
        {view && (
          <div className="p-12 pb-8">
            <Segmented
              aria-label={t("displayOptions.view", "View")}
              options={view.options}
              value={view.value}
              onValueChange={view.onChange}
              fullWidth
              size="sm"
            />
          </div>
        )}
        {rows && rows.length > 0 && (
          <div className="flex flex-col px-12 py-4">
            {rows.map((r) => (
              <div key={r.label} className="flex h-40 items-center justify-between gap-12">
                <span className="flex min-w-0 items-center gap-8 text-body text-fg">
                  {r.icon && <Icon icon={r.icon} size="sm" className="shrink-0 text-fg-secondary" />}
                  <span className="min-w-0 truncate">{r.label}</span>
                </span>
                <Select size="sm" appearance="default" aria-label={r.label} options={r.options} value={r.value} onValueChange={r.onChange} />
              </div>
            ))}
          </div>
        )}
        {toggles && toggles.length > 0 && (
          <div className="flex flex-col px-12 py-4 not-first:hairline-t">
            {toggles.map((toggle) => (
              <Switch key={toggle.label} label={toggle.label} checked={toggle.checked} onCheckedChange={toggle.onChange} className="h-40" />
            ))}
          </div>
        )}
        {properties && (
          <div className="flex flex-col gap-8 p-12 not-first:hairline-t">
            <span className="text-caption font-medium text-fg-secondary">{t("displayOptions.properties", "Display properties")}</span>
            <div className="flex flex-wrap gap-6">
              {properties.options.map((o) => {
                const on = properties.value.includes(o.value);
                const out = on && squeezed.includes(o);
                return (
                  <Tag
                    key={o.value}
                    size="sm"
                    variant={on ? "soft" : "outline"}
                    pressed={on}
                    {...(out ? { icon: icons.hide, "aria-description": t("displayOptions.doesNotFit", "Doesn't fit at this width") } : {})}
                    onClick={() => properties.onChange(on ? properties.value.filter((v) => v !== o.value) : [...properties.value, o.value])}
                  >
                    {o.label}
                  </Tag>
                );
              })}
            </div>
            {squeezed.length > 0 && (
              <span className="text-caption text-fg-tertiary">
                {t(
                  "displayOptions.hiddenProperties",
                  "{count, plural, one {{properties} doesn't fit at this width.} other {{properties} don't fit at this width.}}",
                  {
                    count: squeezed.length,
                    properties: new Intl.ListFormat(locale, { type: "conjunction" }).format(squeezed.map((o) => o.label)),
                  },
                )}
              </span>
            )}
          </div>
        )}
        {footer && <div className="p-12 hairline-t">{footer}</div>}
      </div>
    </RaisedContent>
  );
}

export const DisplayOptions = forwardRef<HTMLElement, DisplayOptionsProps>(function DisplayOptions({ trigger = "secondary", inline, ...props }, ref) {
  const t = useUiText();
  if (!props.view && !props.rows?.length && !props.toggles?.length && !props.properties)
    devWarn("DisplayOptions: pass view, rows, toggles or properties; an empty panel is never useful.");
  if (inline) {
    return (
      <div ref={ref as Ref<HTMLDivElement>} className={cn("rounded-popover bg-raised shadow-raised")}>
        <Panel {...props} inline />
      </div>
    );
  }
  return (
    <Popover>
      <Popover.Trigger>
        <IconButton
          ref={ref as Ref<HTMLButtonElement>}
          icon={icons.displayOptions}
          aria-label={t("displayOptions.displayOptions", "Display options")}
          variant={trigger}
        />
      </Popover.Trigger>
      <Popover.Content padding="none" align="end" aria-label={t("displayOptions.displayOptions", "Display options")}>
        <Panel {...props} />
      </Popover.Content>
    </Popover>
  );
});
DisplayOptions.displayName = "DisplayOptions";
