import { forwardRef, useId, type ReactNode } from "react";
import { Switch as BaseSwitch } from "@base-ui/react/switch";
import { cn } from "../../utils/cn.js";
import { Tooltip } from "../tooltip/tooltip.js";
import { labelled } from "../radio/labelled.js";

export const KAESO_SWITCH_VARIANTS = {
  size: {
    sm: { classes: "h-16 w-28 p-2", description: "28×16, 12px thumb" },
    md: { classes: "h-20 w-36 p-2", description: "36×20, 16px thumb (default)" },
    lg: { classes: "h-24 w-44 p-2", description: "44×24, 20px thumb" },
  },
} as const;
export const KAESO_SWITCH_DEFAULT_VARIANTS = { size: "md" } as const;

const THUMB = {
  sm: "size-12 data-checked:translate-x-12 rtl:data-checked:-translate-x-12",
  md: "size-16 data-checked:translate-x-16 rtl:data-checked:-translate-x-16",
  lg: "size-20 data-checked:translate-x-20 rtl:data-checked:-translate-x-20",
} as const;

export interface SwitchProps {
  checked?: boolean | undefined;
  defaultChecked?: boolean | undefined;
  onCheckedChange?: ((checked: boolean) => void) | undefined;
  disabled?: boolean | undefined;
  disabledReason?: string | undefined;
  readOnly?: boolean | undefined;
  loading?: boolean | undefined;
  name?: string | undefined;
  size?: keyof typeof KAESO_SWITCH_VARIANTS.size | undefined;
  label?: ReactNode | undefined;
  description?: ReactNode | undefined;
  id?: string | undefined;
  "aria-label"?: string | undefined;
  "aria-labelledby"?: string | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

export const Switch = forwardRef<HTMLButtonElement, SwitchProps>(function Switch(
  {
    checked,
    defaultChecked,
    onCheckedChange,
    disabled,
    disabledReason,
    readOnly,
    loading,
    name,
    size = "md",
    label,
    description,
    id,
    className,
    "data-force": force,
    ...aria
  },
  ref,
) {
  const auto = useId();
  const sid = id ?? `k-switch-${auto}`;
  const soft = !!disabledReason && !!disabled;
  const named = labelled(sid, label, label ? description : undefined, aria);
  const switchEl = (
    <BaseSwitch.Root
      ref={ref}
      id={sid}
      {...(checked !== undefined ? { checked } : {})}
      {...(defaultChecked !== undefined ? { defaultChecked } : {})}
      {...(onCheckedChange ? { onCheckedChange: (c: boolean) => onCheckedChange(c) } : {})}
      {...(disabled !== undefined && !soft ? { disabled } : {})}
      {...(readOnly || loading || soft ? { readOnly: true } : {})}
      {...(soft ? { "aria-disabled": true } : {})}
      {...(name ? { name } : {})}
      {...(loading ? { "aria-busy": true } : {})}
      {...(force ? { "data-force": force } : {})}
      {...aria}
      {...named.aria}
      className={cn(
        "group/sw relative inline-flex shrink-0 cursor-pointer items-center rounded-pill bg-fill-3 transition-control focus-ring tap-target",
        "hover:bg-line-strong checked:bg-ink checked:hover:bg-ink",
        "disabled:cursor-not-allowed disabled:opacity-40",
        soft && "cursor-not-allowed opacity-40 hover:bg-fill-3 checked:hover:bg-ink",
        KAESO_SWITCH_VARIANTS.size[size].classes,
        !label && className,
      )}
    >
      <BaseSwitch.Thumb
        className={cn(
          "block rounded-pill bg-raised shadow-thumb indicator-motion data-checked:bg-on-ink data-checked:shadow-none",
          THUMB[size],
          loading && "animate-pulse-quiet",
        )}
      />
    </BaseSwitch.Root>
  );
  const control = soft ? <Tooltip content={disabledReason}>{switchEl}</Tooltip> : switchEl;
  if (!label) return control;
  return (
    <label htmlFor={sid} className={cn("flex min-h-32 cursor-pointer items-center justify-between gap-16", disabled && "cursor-not-allowed", className)}>
      <span className="flex min-w-0 flex-col">
        <span id={named.labelId} className={cn("text-body text-fg", disabled && "text-fg-disabled")}>
          {label}
        </span>
        {description && (
          <span id={named.descriptionId} className="text-small text-fg-secondary">
            {description}
          </span>
        )}
      </span>
      {control}
    </label>
  );
});
Switch.displayName = "Switch";
