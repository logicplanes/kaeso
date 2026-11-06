import { createContext, forwardRef, useContext, useId, type ReactNode } from "react";
import { Checkbox as BaseCheckbox } from "@base-ui/react/checkbox";
import { CheckboxGroup as BaseCheckboxGroup } from "@base-ui/react/checkbox-group";
import { cn } from "../../utils/cn.js";
import { labelled } from "../radio/labelled.js";
import { useField } from "../field/field.js";

export const KAESO_CHECKBOX_VARIANTS = {
  size: {
    md: { classes: "size-18 rounded-checkbox", description: "18px, radius 5 (default)" },
  },
  variant: {
    default: { classes: "", description: "The box with its label beside it (default)" },
    card: {
      classes: "flex min-h-56 cursor-pointer items-center gap-16 px-20 py-12 transition-control",
      description: "A row of a settings group with the box at the end, for an option with a description",
    },
  },
} as const;
export const KAESO_CHECKBOX_DEFAULT_VARIANTS = { size: "md", variant: "default" } as const;

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
  parent?: boolean | undefined;
  variant?: keyof typeof KAESO_CHECKBOX_VARIANTS.variant | undefined;
  id?: string | undefined;
  "aria-label"?: string | undefined;
  "aria-describedby"?: string | undefined;
  "aria-labelledby"?: string | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

const GroupDisabled = createContext(false);
const GroupCards = createContext(false);

export const CheckboxBox = forwardRef<HTMLButtonElement, CheckboxProps>(function CheckboxBox(
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
      data-choice=""
      className={cn(
        "group/cb relative -m-3 inline-flex size-24 shrink-0 cursor-pointer items-center justify-center rounded-checkbox outline-none tap-target disabled:cursor-not-allowed",
        className,
      )}
    >
      <span aria-hidden className={cn("flex items-center justify-center choice-mark", KAESO_CHECKBOX_VARIANTS.size.md.classes)}>
        <BaseCheckbox.Indicator keepMounted className="flex items-center justify-center">
          <svg viewBox="0 0 12 12" className="size-12" fill="none" aria-hidden>
            <path
              d="M2.5 6.25 4.9 8.5 9.5 3.75"
              stroke="currentColor"
              strokeWidth="1.75"
              strokeLinecap="round"
              strokeLinejoin="round"
              pathLength={1}
              className="check-draw"
            />
            <path d="M3 6h6" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" className="hidden group-data-indeterminate/cb:block" />
          </svg>
        </BaseCheckbox.Indicator>
      </span>
    </BaseCheckbox.Root>
  );
});
CheckboxBox.displayName = "CheckboxBox";

const Root = forwardRef<HTMLButtonElement, CheckboxProps>(function Checkbox({ label, description, id, className, variant, ...own }, ref) {
  const auto = useId();
  const boxId = id ?? `k-cb-${auto}`;
  const field = useField();
  const groupDisabled = useContext(GroupDisabled);
  const groupCards = useContext(GroupCards);
  const fieldDescribedBy = [own["aria-describedby"], field?.descriptionId, field?.errorId].filter(Boolean).join(" ");
  const props = {
    ...own,
    ...(field?.invalid && own.invalid === undefined ? { invalid: true } : {}),
    ...((field?.disabled || groupDisabled) && own.disabled === undefined ? { disabled: true } : {}),
    ...(fieldDescribedBy ? { "aria-describedby": fieldDescribedBy } : {}),
  };
  if (!label) return <CheckboxBox ref={ref} id={boxId} className={className} {...props} />;
  const named = labelled(boxId, label, description, props);
  if (variant === "card" || (variant === undefined && groupCards)) {
    const off = !!props.disabled;
    return (
      <label
        htmlFor={boxId}
        data-choice-row=""
        {...(props["data-force"] ? { "data-force": props["data-force"] } : {})}
        className={cn(
          KAESO_CHECKBOX_VARIANTS.variant.card.classes,
          groupCards ? "not-first:hairline-t" : "rounded-group bg-raised shadow-xs",
          off ? "cursor-not-allowed" : "hover:bg-raised-hover active:bg-raised-pressed focus-ring-card-row-within",
          className,
        )}
      >
        <span className="flex min-w-0 flex-1 flex-col gap-2">
          <span id={named.labelId} className={cn("text-body-lg font-medium text-fg", off && "text-fg-disabled")}>
            {label}
          </span>
          {description && (
            <span id={named.descriptionId} className={cn("text-small text-pretty", off ? "text-fg-disabled" : "text-fg-secondary")}>
              {description}
            </span>
          )}
        </span>
        <CheckboxBox ref={ref} id={boxId} {...props} {...named.aria} />
      </label>
    );
  }
  return (
    <label
      htmlFor={boxId}
      data-choice-row=""
      className={cn(
        "inline-flex min-h-32 cursor-pointer items-start gap-10 py-6 select-none coarse:min-h-44 coarse:py-12",
        props.disabled && "cursor-not-allowed",
        className,
      )}
    >
      <span className="flex h-20 items-center">
        <CheckboxBox ref={ref} id={boxId} {...props} {...named.aria} />
      </span>
      <span className="flex flex-col">
        <span id={named.labelId} className={cn("text-body text-fg", props.disabled && "text-fg-disabled")}>
          {label}
        </span>
        {description && (
          <span id={named.descriptionId} className={cn("text-small text-pretty", props.disabled ? "text-fg-disabled" : "text-fg-secondary")}>
            {description}
          </span>
        )}
      </span>
    </label>
  );
});
Root.displayName = "Checkbox";

export const KAESO_CHECKBOX_GROUP_VARIANTS = {
  variant: {
    default: { classes: "flex flex-col", description: "A list of checkboxes (default)" },
    cards: {
      classes: "flex flex-col overflow-hidden rounded-group bg-raised shadow-xs",
      description: "Rows inside a settings group with the box at the end, for options with descriptions",
    },
  },
} as const;
export const KAESO_CHECKBOX_GROUP_DEFAULT_VARIANTS = { variant: "default" } as const;

export interface CheckboxGroupProps {
  value?: string[] | undefined;
  defaultValue?: string[] | undefined;
  onValueChange?: ((value: string[]) => void) | undefined;
  allValues?: string[] | undefined;
  disabled?: boolean | undefined;
  variant?: keyof typeof KAESO_CHECKBOX_GROUP_VARIANTS.variant | undefined;
  "aria-label"?: string | undefined;
  "aria-labelledby"?: string | undefined;
  className?: string | undefined;
  children: ReactNode;
}

const Group = forwardRef<HTMLDivElement, CheckboxGroupProps>(function CheckboxGroup(
  { value, defaultValue, onValueChange, allValues, disabled, variant = "default", className, children, ...props },
  ref,
) {
  return (
    <GroupDisabled.Provider value={!!disabled}>
      <GroupCards.Provider value={variant === "cards"}>
        <BaseCheckboxGroup
          ref={ref}
          {...(value !== undefined ? { value } : {})}
          {...(defaultValue !== undefined ? { defaultValue } : {})}
          {...(onValueChange ? { onValueChange: (v: string[]) => onValueChange(v) } : {})}
          {...(allValues ? { allValues } : {})}
          {...(disabled !== undefined ? { disabled } : {})}
          className={cn(KAESO_CHECKBOX_GROUP_VARIANTS.variant[variant].classes, className)}
          {...props}
        >
          {children}
        </BaseCheckboxGroup>
      </GroupCards.Provider>
    </GroupDisabled.Provider>
  );
});
Group.displayName = "CheckboxGroup";

export const Checkbox = Root;
export const CheckboxGroup = Group;
