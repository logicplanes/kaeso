import { createContext, forwardRef, useContext, useId, type FieldsetHTMLAttributes, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn";
import { useUiText } from "../../i18n/context";

export const KAESO_FIELD_VARIANTS = {
  layout: {
    vertical: { classes: "flex flex-col gap-8", description: "Label above the control (default)" },
    horizontal: {
      classes: "grid grid-cols-1 gap-8 md:grid-cols-label md:gap-x-24",
      description: "Label in a 240px column on the left, from 768px",
    },
  },
} as const;
export const KAESO_FIELD_DEFAULT_VARIANTS = { layout: "vertical" } as const;

export interface FieldContextValue {
  controlId: string;
  labelId: string;
  descriptionId: string | undefined;
  errorId: string | undefined;
  invalid: boolean;
  disabled: boolean;
  required: boolean;
  controlProps: { id: string; "aria-describedby"?: string; "aria-invalid"?: true; "aria-required"?: true };
}

const FieldContext = createContext<FieldContextValue | null>(null);
export const useField = () => useContext(FieldContext);

export interface FieldProps extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  label?: ReactNode | undefined;
  description?: ReactNode | undefined;
  invalid?: boolean | undefined;
  error?: ReactNode | undefined;
  optional?: boolean | undefined;
  required?: boolean | undefined;
  disabled?: boolean | undefined;
  layout?: keyof typeof KAESO_FIELD_VARIANTS.layout | undefined;
  id?: string | undefined;
  children: ReactNode;
}

export const Field = forwardRef<HTMLDivElement, FieldProps>(function Field(
  { label, description, invalid: invalidProp, error, optional, required = false, disabled = false, layout = "vertical", id, className, children, ...props },
  ref,
) {
  const auto = useId();
  const t = useUiText();
  const controlId = id ?? `k-field-${auto}`;
  const labelId = `${controlId}-label`;
  const descriptionId = description ? `${controlId}-description` : undefined;
  const errorId = error ? `${controlId}-error` : undefined;
  const invalid = !!invalidProp || !!error;
  const describedBy = [descriptionId, errorId].filter(Boolean).join(" ") || undefined;
  const value: FieldContextValue = {
    controlId,
    labelId,
    descriptionId,
    errorId,
    invalid,
    disabled,
    required,
    controlProps: {
      id: controlId,
      ...(describedBy ? { "aria-describedby": describedBy } : {}),
      ...(invalid ? { "aria-invalid": true as const } : {}),
      ...(required ? { "aria-required": true as const } : {}),
    },
  };
  return (
    <FieldContext.Provider value={value}>
      <div ref={ref} className={cn(KAESO_FIELD_VARIANTS.layout[layout].classes, className)} data-disabled={disabled || undefined} {...props}>
        {label && (
          <label
            id={labelId}
            htmlFor={controlId}
            className={cn("flex items-baseline gap-6 text-body font-medium text-fg", layout === "horizontal" && "md:pt-8", disabled && "text-fg-disabled")}
          >
            {label}
            {optional && <span className="text-small font-regular text-fg-tertiary">{t("field.optional", "Optional")}</span>}
          </label>
        )}
        <div className="flex min-w-0 flex-col gap-6">
          {children}
          {description && (
            <p id={descriptionId} className="text-small text-fg-secondary">
              {description}
            </p>
          )}
          {error && (
            <p id={errorId} className="sr-only">
              {error}
            </p>
          )}
        </div>
      </div>
    </FieldContext.Provider>
  );
});
Field.displayName = "Field";

export interface FieldsetProps extends FieldsetHTMLAttributes<HTMLFieldSetElement> {
  legend: ReactNode;
  description?: ReactNode | undefined;
}

export const Fieldset = forwardRef<HTMLFieldSetElement, FieldsetProps>(function Fieldset({ legend, description, className, children, ...props }, ref) {
  return (
    <fieldset ref={ref} className={cn("flex min-w-0 flex-col gap-16", className)} {...props}>
      <div className="flex flex-col gap-4">
        <legend className="text-body-lg font-semibold text-fg">{legend}</legend>
        {description && <p className="text-small text-fg-secondary">{description}</p>}
      </div>
      {children}
    </fieldset>
  );
});
Fieldset.displayName = "Fieldset";

export function useFieldControl(props: {
  label?: ReactNode;
  description?: ReactNode;
  error?: ReactNode;
  invalid?: boolean | undefined;
  required?: boolean | undefined;
  id?: string | undefined;
}) {
  const field = useField();
  const auto = useId();
  if (field) return { controlProps: field.controlProps as Record<string, unknown>, wrapId: undefined };
  const wraps = props.label !== undefined || props.description !== undefined || props.error !== undefined;
  if (!wraps) return { controlProps: (props.invalid ? { "aria-invalid": true } : {}) as Record<string, unknown>, wrapId: undefined };
  const controlId = props.id ?? `k-field-${auto}`;
  const describedBy = [props.description ? `${controlId}-description` : "", props.error ? `${controlId}-error` : ""].filter(Boolean).join(" ");
  return {
    controlProps: {
      id: controlId,
      ...(describedBy ? { "aria-describedby": describedBy } : {}),
      ...(props.error || props.invalid ? { "aria-invalid": true } : {}),
      ...(props.required ? { "aria-required": true } : {}),
    } as Record<string, unknown>,
    wrapId: controlId,
  };
}

export interface FieldWrapProps {
  label?: ReactNode | undefined;
  description?: ReactNode | undefined;
  error?: ReactNode | undefined;
  optional?: boolean | undefined;
  required?: boolean | undefined;
  layout?: keyof typeof KAESO_FIELD_VARIANTS.layout | undefined;
}

export function withField(
  props: FieldWrapProps & { id?: string | undefined; disabled?: boolean | undefined; invalid?: boolean | undefined; className?: string | undefined },
  control: ReactNode,
) {
  const { label, description, error, optional, required, layout } = props;
  if (label === undefined && description === undefined && error === undefined) return control;
  return (
    <Field
      label={label}
      description={description}
      error={error}
      invalid={props.invalid}
      optional={optional}
      required={required}
      layout={layout}
      id={props.id}
      disabled={props.disabled}
      className={props.className}
    >
      {control}
    </Field>
  );
}
