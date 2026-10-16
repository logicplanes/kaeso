import { forwardRef, Fragment, useId } from "react";
import { RadioGroup as BaseRadioGroup } from "@base-ui/react/radio-group";
import { cn } from "../../utils/cn.js";
import { RadioCircle } from "../radio/radio.js";
import { Select } from "../select/select.js";
import { Tooltip } from "../tooltip/tooltip.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_ROLE_PICKER_VARIANTS = {
  appearance: {
    list: {
      classes: "flex flex-col gap-16",
      description: "Every role as a row with its description, built-in then custom (default): invites and role changes",
    },
    select: { classes: "", description: "A pill that opens the roles: filters, table rows and tight forms" },
  },
} as const;
export const KAESO_ROLE_PICKER_DEFAULT_VARIANTS = { appearance: "list" } as const;

export interface RoleOption {
  value: string;
  label: string;
  description?: string | undefined;
  custom?: boolean | undefined;
  hint?: string | undefined;
  disabled?: boolean | undefined;
  disabledReason?: string | undefined;
}

export interface RolePickerProps {
  roles: RoleOption[];
  value?: string | null | undefined;
  onValueChange?: ((value: string) => void) | undefined;
  appearance?: keyof typeof KAESO_ROLE_PICKER_VARIANTS.appearance | undefined;
  disabled?: boolean | undefined;
  name?: string | undefined;
  label?: string | undefined;
  "aria-label"?: string | undefined;
  fullWidth?: boolean | undefined;
  size?: "sm" | "md" | undefined;
  className?: string | undefined;
}

export const RolePicker = forwardRef<HTMLDivElement, RolePickerProps>(function RolePicker(
  { roles, value, onValueChange, appearance = "list", disabled, name, label, "aria-label": ariaLabel, fullWidth, size = "md", className },
  ref,
) {
  const t = useUiText();
  const baseId = useId();
  const builtIn = t("rolePicker.builtInRoles", "Built-in roles");
  const custom = t("rolePicker.customRoles", "Custom roles");
  const hasCustom = roles.some((r) => r.custom);
  if (appearance === "select") {
    return (
      <div ref={ref} className={cn("min-w-0", fullWidth && "w-full", className)}>
        <Select
          {...(label ? { label } : {})}
          {...(ariaLabel ? { "aria-label": ariaLabel } : {})}
          {...(name ? { name } : {})}
          {...(disabled !== undefined ? { disabled } : {})}
          {...(fullWidth ? { fullWidth } : {})}
          size={size}
          value={value ?? null}
          {...(onValueChange ? { onValueChange } : {})}
          options={[...roles.filter((r) => !r.custom), ...roles.filter((r) => r.custom)].map((r) => ({
            value: r.value,
            label: r.label,
            textValue: r.label,
            description: r.disabled && r.disabledReason ? r.disabledReason : r.description,
            ...(hasCustom ? { group: r.custom ? custom : builtIn } : {}),
            ...(r.disabled ? { disabled: true } : {}),
          }))}
        />
      </div>
    );
  }
  const sections = hasCustom
    ? [
        { caption: builtIn, items: roles.filter((r) => !r.custom) },
        { caption: custom, items: roles.filter((r) => r.custom) },
      ]
    : [{ caption: undefined, items: roles }];
  return (
    <BaseRadioGroup
      ref={ref}
      {...(value != null ? { value } : {})}
      {...(onValueChange ? { onValueChange: (v: unknown) => onValueChange(String(v)) } : {})}
      {...(disabled !== undefined ? { disabled } : {})}
      {...(name ? { name } : {})}
      aria-label={ariaLabel ?? label ?? t("rolePicker.role", "Role")}
      className={cn(KAESO_ROLE_PICKER_VARIANTS.appearance.list.classes, className)}
    >
      {sections
        .filter((s) => s.items.length > 0)
        .map((s, si) => (
          <Fragment key={s.caption ?? "all"}>
            <div className="flex flex-col gap-8">
              {s.caption && <span className="px-4 text-caption font-medium text-fg-secondary">{s.caption}</span>}
              <div className="flex flex-col overflow-hidden rounded-group bg-raised shadow-xs">
                {s.items.map((r, i) => {
                  const id = `${baseId}-${si}-${i}`;
                  const off = disabled || r.disabled;
                  const row = (
                    <label
                      htmlFor={id}
                      className={cn(
                        "flex min-h-56 cursor-pointer items-center gap-16 px-16 py-12 transition-control hover:bg-fill-1 not-first:hairline-t",
                        off && "cursor-not-allowed hover:bg-transparent",
                      )}
                    >
                      <span className="flex min-w-0 flex-1 flex-col gap-2">
                        <span className="flex min-w-0 items-baseline gap-8">
                          <span id={`${id}-name`} className={cn("min-w-0 wrap-break-word text-body font-medium", off ? "text-fg-disabled" : "text-fg")}>
                            {r.label}
                          </span>
                          {r.hint && <span className="ml-auto shrink-0 text-small text-fg-tertiary tabular">{r.hint}</span>}
                        </span>
                        {r.description && (
                          <span id={`${id}-desc`} className={cn("text-small text-pretty", off ? "text-fg-tertiary" : "text-fg-secondary")}>
                            {r.description}
                          </span>
                        )}
                        {r.disabled && r.disabledReason && <span className="text-caption text-fg-tertiary">{r.disabledReason}</span>}
                      </span>
                      <RadioCircle
                        id={id}
                        value={r.value}
                        {...(r.disabled ? { disabled: true } : {})}
                        aria-labelledby={`${id}-name`}
                        {...(r.description ? { "aria-describedby": `${id}-desc` } : {})}
                      />
                    </label>
                  );
                  return r.disabled && r.disabledReason ? (
                    <Tooltip key={r.value} content={r.disabledReason}>
                      {row}
                    </Tooltip>
                  ) : (
                    <Fragment key={r.value}>{row}</Fragment>
                  );
                })}
              </div>
            </div>
          </Fragment>
        ))}
    </BaseRadioGroup>
  );
});
RolePicker.displayName = "RolePicker";
