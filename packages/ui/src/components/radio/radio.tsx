import { createContext, forwardRef, useContext, useId, type ReactNode } from "react";
import { Radio as BaseRadio } from "@base-ui/react/radio";
import { RadioGroup as BaseRadioGroup } from "@base-ui/react/radio-group";
import { cn } from "../../utils/cn.js";
import { labelled } from "./labelled.js";

export const KAESO_RADIO_VARIANTS = {
  variant: {
    default: { classes: "flex flex-col", description: "A list of radios with labels (default)" },
    cards: {
      classes: "flex flex-col overflow-hidden rounded-group bg-raised shadow-xs",
      description: "Rows inside a settings group with the radio at the end, for choices with descriptions",
    },
  },
} as const;
export const KAESO_RADIO_DEFAULT_VARIANTS = { variant: "default" } as const;

const Ctx = createContext<{ variant: keyof typeof KAESO_RADIO_VARIANTS.variant; disabled: boolean }>({ variant: "default", disabled: false });

export interface RadioGroupProps {
  value?: string | undefined;
  defaultValue?: string | undefined;
  onValueChange?: ((value: string) => void) | undefined;
  disabled?: boolean | undefined;
  name?: string | undefined;
  variant?: keyof typeof KAESO_RADIO_VARIANTS.variant | undefined;
  orientation?: "vertical" | "horizontal" | undefined;
  "aria-label"?: string | undefined;
  "aria-labelledby"?: string | undefined;
  className?: string | undefined;
  children: ReactNode;
}

const Group = forwardRef<HTMLDivElement, RadioGroupProps>(function RadioGroup(
  { value, defaultValue, onValueChange, disabled, name, variant = "default", orientation = "vertical", className, children, ...props },
  ref,
) {
  return (
    <Ctx.Provider value={{ variant, disabled: !!disabled }}>
      <BaseRadioGroup
        ref={ref}
        {...(value !== undefined ? { value } : {})}
        {...(defaultValue !== undefined ? { defaultValue } : {})}
        {...(onValueChange ? { onValueChange: (v: unknown) => onValueChange(String(v)) } : {})}
        {...(disabled !== undefined ? { disabled } : {})}
        {...(name ? { name } : {})}
        className={cn(
          KAESO_RADIO_VARIANTS.variant[variant].classes,
          orientation === "horizontal" && variant === "default" && "flex-row flex-wrap gap-x-24",
          className,
        )}
        {...props}
      >
        {children}
      </BaseRadioGroup>
    </Ctx.Provider>
  );
});
Group.displayName = "RadioGroup";

export interface RadioProps {
  value: string;
  label?: ReactNode | undefined;
  description?: ReactNode | undefined;
  disabled?: boolean | undefined;
  id?: string | undefined;
  "aria-label"?: string | undefined;
  "aria-labelledby"?: string | undefined;
  "aria-describedby"?: string | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

export const RadioCircle = forwardRef<HTMLButtonElement, RadioProps>(function RadioCircle(
  { value, disabled, id, className, "aria-label": ariaLabel, "aria-labelledby": labelledBy, "aria-describedby": describedBy, "data-force": force },
  ref,
) {
  return (
    <BaseRadio.Root
      ref={ref}
      value={value}
      {...(disabled !== undefined ? { disabled } : {})}
      {...(id ? { id } : {})}
      {...(ariaLabel ? { "aria-label": ariaLabel } : {})}
      {...(labelledBy ? { "aria-labelledby": labelledBy } : {})}
      {...(describedBy ? { "aria-describedby": describedBy } : {})}
      {...(force ? { "data-force": force } : {})}
      data-choice=""
      className={cn(
        "relative -m-3 inline-flex size-24 shrink-0 cursor-pointer items-center justify-center rounded-pill outline-none tap-target disabled:cursor-not-allowed",
        className,
      )}
    >
      <span aria-hidden data-shape="dot" className="size-18 rounded-pill choice-mark" />
    </BaseRadio.Root>
  );
});
RadioCircle.displayName = "Radio.Circle";

const Root = forwardRef<HTMLButtonElement, RadioProps>(function Radio({ label, description, id, className, ...props }, ref) {
  const { variant, disabled: groupDisabled } = useContext(Ctx);
  const off = !!props.disabled || groupDisabled;
  const auto = useId();
  const rid = id ?? `k-radio-${auto}`;
  const named = labelled(rid, label, description, props);
  if (variant === "cards") {
    return (
      <label
        htmlFor={rid}
        data-choice-row=""
        {...(props["data-force"] ? { "data-force": props["data-force"] } : {})}
        className={cn(
          "flex min-h-56 cursor-pointer items-center gap-16 px-20 py-12 transition-control not-first:hairline-t",
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
        <RadioCircle ref={ref} id={rid} {...props} {...named.aria} />
      </label>
    );
  }
  if (!label) return <RadioCircle ref={ref} id={rid} className={className} {...props} />;
  return (
    <label
      htmlFor={rid}
      data-choice-row=""
      className={cn(
        "inline-flex min-h-32 cursor-pointer items-start gap-10 py-6 select-none coarse:min-h-44 coarse:py-12",
        off && "cursor-not-allowed",
        className,
      )}
    >
      <span className="flex h-20 items-center">
        <RadioCircle ref={ref} id={rid} {...props} {...named.aria} />
      </span>
      <span className="flex flex-col">
        <span id={named.labelId} className={cn("text-body text-fg", off && "text-fg-disabled")}>
          {label}
        </span>
        {description && (
          <span id={named.descriptionId} className={cn("text-small text-pretty", off ? "text-fg-disabled" : "text-fg-secondary")}>
            {description}
          </span>
        )}
      </span>
    </label>
  );
});
Root.displayName = "Radio";

export const Radio = Object.assign(Root, { Group, Circle: RadioCircle });
export const RadioGroup = Group;
export type RadioCardsProps = Omit<RadioGroupProps, "variant">;

export const RadioCards = forwardRef<HTMLDivElement, RadioCardsProps>(function RadioCards(props, ref) {
  return <Group ref={ref} variant="cards" {...props} />;
});
RadioCards.displayName = "RadioCards";
