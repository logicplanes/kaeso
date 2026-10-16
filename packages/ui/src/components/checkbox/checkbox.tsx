import { forwardRef, useId, type ReactNode } from "react";
import { Checkbox as BaseCheckbox } from "@base-ui/react/checkbox";
import { CheckboxGroup as BaseCheckboxGroup } from "@base-ui/react/checkbox-group";
import { cn } from "../../utils/cn.js";
import { labelled } from "../radio/labelled.js";

export const KAESO_CHECKBOX_VARIANTS = {
  size: {
    md: { classes: "size-18 rounded-checkbox", description: "18px, radius 5 (default)" },
  },
} as const;
export const KAESO_CHECKBOX_DEFAULT_VARIANTS = { size: "md" } as const;

export interface CheckboxProps {
  checked?: boolean | undefined;
  defaultChecked?: boolean | undefined;
  onCheckedChange?: ((checked: boolean) => void) | undefined;
  indeterminate?: boolean | undefined;
  disabled?: boolean | undefined;
  readOnly?: boolean | undefined;
  required?: boolean | undefined;
  invalid?: boolean | undefined;
  name?: string | undefined;
  value?: string | undefined;
  label?: ReactNode | undefined;
  description?: ReactNode | undefined;
  id?: string | undefined;
  "aria-label"?: string | undefined;
  "aria-describedby"?: string | undefined;
  "aria-labelledby"?: string | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

export const CheckboxBox = forwardRef<HTMLButtonElement, CheckboxProps & { parent?: boolean | undefined }>(function CheckboxBox(
  {
    checked,
    defaultChecked,
    onCheckedChange,
    indeterminate,
    disabled,
    readOnly,
    required,
    invalid,
    name,
    value,
    id,
    className,
    parent,
    "aria-label": ariaLabel,
    "aria-describedby": describedBy,
    "aria-labelledby": labelledBy,
    "data-force": force,
  },
  ref,
) {
  return (
    <BaseCheckbox.Root
      ref={ref}
      {...(checked !== undefined ? { checked } : {})}
      {...(defaultChecked !== undefined ? { defaultChecked } : {})}
      {...(onCheckedChange ? { onCheckedChange: (c: boolean) => onCheckedChange(c) } : {})}
      {...(indeterminate !== undefined ? { indeterminate } : {})}
      {...(disabled !== undefined ? { disabled } : {})}
      {...(readOnly !== undefined ? { readOnly } : {})}
      {...(required !== undefined ? { required } : {})}
      {...(name ? { name } : {})}
      {...(value ? { value } : {})}
      {...(id ? { id } : {})}
      {...(parent ? { parent } : {})}
      {...(ariaLabel ? { "aria-label": ariaLabel } : {})}
      {...(describedBy ? { "aria-describedby": describedBy } : {})}
      {...(labelledBy ? { "aria-labelledby": labelledBy } : {})}
      {...(invalid ? { "aria-invalid": true } : {})}
      {...(force ? { "data-force": force } : {})}
      className={cn(
        "group/cb relative -m-3 inline-flex size-24 shrink-0 cursor-pointer items-center justify-center rounded-checkbox outline-none tap-target disabled:cursor-not-allowed",
        className,
      )}
    >
      <span
        aria-hidden
        className={cn(
          "flex items-center justify-center bg-raised shadow-selector outline-2 outline-offset-2 outline-transparent transition-control",
          KAESO_CHECKBOX_VARIANTS.size.md.classes,
          invalid ? "group-focus-visible/cb:outline-focus" : "group-hover/cb:shadow-selector-hover group-focus-visible/cb:outline-focus",
          "group-checked/cb:bg-ink group-checked/cb:shadow-none group-indeterminate/cb:bg-ink group-indeterminate/cb:shadow-none",
          "group-invalid/cb:ring-invalid",
          "group-disabled/cb:bg-fill-1 group-disabled/cb:shadow-none group-disabled/cb:group-checked/cb:bg-fill-3",
        )}
      >
        <BaseCheckbox.Indicator keepMounted className="flex items-center justify-center text-on-ink group-data-disabled/cb:text-fg-disabled">
          <svg viewBox="0 0 12 12" className="size-12" fill="none" aria-hidden>
            <path
              d="M2.5 6.25 4.9 8.5 9.5 3.75"
              stroke="currentColor"
              strokeWidth="1.75"
              strokeLinecap="round"
              strokeLinejoin="round"
              pathLength={1}
              className="check-draw hidden group-data-checked/cb:block"
            />
            <path d="M3 6h6" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" className="hidden group-data-indeterminate/cb:block" />
          </svg>
        </BaseCheckbox.Indicator>
      </span>
    </BaseCheckbox.Root>
  );
});
CheckboxBox.displayName = "Checkbox.Box";

const Root = forwardRef<HTMLButtonElement, CheckboxProps>(function Checkbox({ label, description, id, className, ...props }, ref) {
  const auto = useId();
  const boxId = id ?? `k-cb-${auto}`;
  if (!label) return <CheckboxBox ref={ref} id={boxId} className={className} {...props} />;
  const named = labelled(boxId, label, description, props);
  return (
    <label
      htmlFor={boxId}
      className={cn("group/row inline-flex min-h-32 cursor-pointer items-start gap-10 py-6 select-none", props.disabled && "cursor-not-allowed", className)}
    >
      <span className="flex h-20 items-center">
        <CheckboxBox ref={ref} id={boxId} {...props} {...named.aria} />
      </span>
      <span className="flex flex-col">
        <span id={named.labelId} className={cn("text-body text-fg", props.disabled && "text-fg-disabled")}>
          {label}
        </span>
        {description && (
          <span id={named.descriptionId} className="text-small text-fg-secondary">
            {description}
          </span>
        )}
      </span>
    </label>
  );
});
Root.displayName = "Checkbox";

export interface CheckboxGroupProps {
  value?: string[] | undefined;
  defaultValue?: string[] | undefined;
  onValueChange?: ((value: string[]) => void) | undefined;
  allValues?: string[] | undefined;
  disabled?: boolean | undefined;
  "aria-label"?: string | undefined;
  "aria-labelledby"?: string | undefined;
  className?: string | undefined;
  children: ReactNode;
}

const Group = forwardRef<HTMLDivElement, CheckboxGroupProps>(function CheckboxGroup(
  { value, defaultValue, onValueChange, allValues, disabled, className, children, ...props },
  ref,
) {
  return (
    <BaseCheckboxGroup
      ref={ref}
      {...(value !== undefined ? { value } : {})}
      {...(defaultValue !== undefined ? { defaultValue } : {})}
      {...(onValueChange ? { onValueChange: (v: string[]) => onValueChange(v) } : {})}
      {...(allValues ? { allValues } : {})}
      {...(disabled !== undefined ? { disabled } : {})}
      className={cn("flex flex-col", className)}
      {...props}
    >
      {children}
    </BaseCheckboxGroup>
  );
});
Group.displayName = "CheckboxGroup";

export const Checkbox = Object.assign(Root, { Box: CheckboxBox, Group });
export const CheckboxGroup = Group;
