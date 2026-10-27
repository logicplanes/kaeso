import { pageBoundary, usePortalContainer } from "../../utils/portal-context.js";
import { Fragment, forwardRef, useCallback, useRef, type ReactNode } from "react";
import { CSPProvider } from "@base-ui/react/csp-provider";
import { Select as BaseSelect } from "@base-ui/react/select";
import { cn } from "../../utils/cn.js";
import { warnStylingClassName } from "../../utils/dev.js";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field.js";
import { Icon, type IconData } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";
import { MENU_ICON, MENU_ITEM, MENU_LABEL, MENU_POPUP, MENU_SEPARATOR } from "../menu/styles.js";
import { anchorWidthStyle, useLayoutWidth } from "../../hooks/use-layout-width.js";
import { useUiText } from "../../i18n/context.js";
import { OptionCheckbox } from "./option-checkbox.js";

export const KAESO_SELECT_VARIANTS = {
  appearance: {
    default: { classes: "material-selector", description: "A pill with the same ring, hover and focus as a text input (default)" },
    ghost: {
      classes: "material-ghost focus-ring text-fg-secondary hover:text-fg focus-visible:text-fg open:text-fg",
      description: "Inline in settings rows: “GPT-5 mini (default) ⌄”",
    },
    header: { classes: "material-ghost focus-ring text-fg", description: "Transparent until hovered, in full text colour: a column header's model" },
  },
  size: {
    sm: { classes: "h-32 pl-12 pr-10 gap-6 text-body", description: "32px: settings rows and toolbars" },
    md: { classes: "h-36 pl-16 pr-12 gap-8 text-body", description: "36px (default)" },
    lg: { classes: "h-40 pl-18 pr-14 gap-8 text-body", description: "40px" },
    xl: { classes: "h-48 pl-22 pr-18 gap-8 text-body-lg", description: "48px: sign-in and setup forms" },
  },
} as const;
export const KAESO_SELECT_DEFAULT_VARIANTS = { appearance: "default", size: "md" } as const;

export interface SelectOption {
  value: string;
  label: ReactNode;
  textValue?: string | undefined;
  description?: ReactNode | undefined;
  icon?: IconData | undefined;
  group?: string | undefined;
  disabled?: boolean | undefined;
}

interface BaseProps extends FieldWrapProps {
  options: SelectOption[];
  placeholder?: ReactNode | undefined;
  size?: keyof typeof KAESO_SELECT_VARIANTS.size | undefined;
  appearance?: keyof typeof KAESO_SELECT_VARIANTS.appearance | undefined;
  disabled?: boolean | undefined;
  name?: string | undefined;
  id?: string | undefined;
  open?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  invalid?: boolean | undefined;
  icon?: IconData | undefined;
  fullWidth?: boolean | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}
export interface SingleSelectProps extends BaseProps {
  multiple?: false | undefined;
  value?: string | null | undefined;
  defaultValue?: string | null | undefined;
  onValueChange?: ((value: string) => void) | undefined;
}
export interface MultiSelectProps extends BaseProps {
  multiple: true;
  value?: string[] | undefined;
  defaultValue?: string[] | undefined;
  onValueChange?: ((value: string[]) => void) | undefined;
}
export type SelectProps = SingleSelectProps | MultiSelectProps;

function changeWith<T extends string | string[]>(onChange: (value: T) => void, accepts: (value: unknown) => value is T) {
  return (value: unknown) => {
    if (accepts(value)) onChange(value);
  };
}

const SCROLL_ARROW = "absolute inset-x-0 z-10 flex h-24 cursor-default items-center justify-center bg-raised text-fg-tertiary";

function groupsOf(options: SelectOption[]) {
  const out: { group: string | undefined; items: SelectOption[] }[] = [];
  for (const o of options) {
    const last = out[out.length - 1];
    if (last && last.group === o.group) last.items.push(o);
    else out.push({ group: o.group, items: [o] });
  }
  return out;
}

export const Select = forwardRef<HTMLButtonElement, SelectProps>(function Select(props, ref) {
  const {
    options,
    placeholder: placeholderProp,
    size = "md",
    appearance = "default",
    disabled,
    name,
    id,
    open,
    onOpenChange,
    invalid,
    icon,
    fullWidth,
    label,
    description,
    error,
    optional,
    required,
    layout,
    className,
    multiple,
    "aria-label": ariaLabel,
    "data-force": force,
  } = props;
  warnStylingClassName("Select", className);
  const t = useUiText();
  const placeholder = placeholderProp ?? t("select.placeholder", "Not set");
  const field = useField();
  const fc = useFieldControl(props);
  const trigger = useRef<HTMLButtonElement | null>(null);
  const triggerWidth = useLayoutWidth(trigger);
  const setTrigger = useCallback(
    (el: HTMLButtonElement | null) => {
      trigger.current = el;
      if (typeof ref === "function") ref(el);
      else if (ref) ref.current = el;
    },
    [ref],
  );
  const byValue = new Map(options.map((o) => [o.value, o]));
  const textOf = (v: string) => {
    const o = byValue.get(v);
    return o?.textValue ?? (typeof o?.label === "string" ? o.label : v);
  };
  const valueProps: Pick<BaseSelect.Root.Props<string, boolean>, "multiple" | "value" | "defaultValue" | "onValueChange"> = props.multiple
    ? {
        multiple: true,
        ...(props.value !== undefined ? { value: props.value } : {}),
        ...(props.defaultValue !== undefined ? { defaultValue: props.defaultValue } : {}),
        ...(props.onValueChange ? { onValueChange: changeWith(props.onValueChange, Array.isArray) } : {}),
      }
    : {
        ...(props.value !== undefined ? { value: props.value } : {}),
        ...(props.defaultValue !== undefined ? { defaultValue: props.defaultValue } : {}),
        ...(props.onValueChange ? { onValueChange: changeWith(props.onValueChange, (v) => typeof v === "string") } : {}),
      };
  const control = (
    <BaseSelect.Root
      {...valueProps}
      {...((field?.required ?? required) ? { required: true } : {})}
      {...(disabled !== undefined ? { disabled } : {})}
      {...(name ? { name } : {})}
      {...(open !== undefined ? { open } : {})}
      {...(onOpenChange ? { onOpenChange: (o: boolean) => onOpenChange(o) } : {})}
      itemToStringLabel={(v: unknown) => textOf(String(v))}
      modal={false}
    >
      <BaseSelect.Trigger
        ref={setTrigger}
        {...(id ? { id } : {})}
        {...fc.controlProps}
        {...(ariaLabel ? { "aria-label": ariaLabel } : {})}
        {...(invalid || error ? { "aria-invalid": true } : {})}
        {...(force ? { "data-force": force } : {})}
        className={cn(
          "group/trigger inline-flex min-w-0 cursor-pointer items-center justify-between rounded-pill font-medium whitespace-nowrap transition-control disabled:cursor-not-allowed",
          KAESO_SELECT_VARIANTS.size[size].classes,
          KAESO_SELECT_VARIANTS.appearance[appearance].classes,
          (invalid || error) && appearance !== "default" && "ring-invalid",
          fullWidth && "w-full",
          !(label || description || error) && className,
        )}
      >
        {icon && <Icon icon={icon} size="sm" className="text-fg-secondary group-disabled/trigger:text-fg-disabled" />}
        <BaseSelect.Value
          className={cn(
            "min-w-0 flex-1 truncate text-left data-placeholder:font-regular data-placeholder:text-fg-tertiary group-disabled/trigger:data-placeholder:text-fg-disabled",
            multiple && "flex items-center",
          )}
          placeholder={placeholder}
        >
          {(value: unknown) => {
            if (multiple) {
              const arr = Array.isArray(value) ? value.map(String) : [];
              const [first] = arr;
              if (first === undefined) return placeholder;
              if (arr.length === 1) return <span className="min-w-0 truncate">{textOf(first)}</span>;
              return (
                <>
                  <span className="min-w-0 truncate">{textOf(first)}</span>{" "}
                  <span className="shrink-0 pl-4 tabular">{t("select.more", "+{count}", { count: String(arr.length - 1) })}</span>
                </>
              );
            }
            return value == null || value === "" ? placeholder : (byValue.get(String(value))?.label ?? String(value));
          }}
        </BaseSelect.Value>
        <BaseSelect.Icon className="flex shrink-0 text-fg-tertiary group-disabled/trigger:text-fg-disabled">
          <Icon icon={icons.chevronDown} size="sm" />
        </BaseSelect.Icon>
      </BaseSelect.Trigger>
      <CSPProvider disableStyleElements>
        <BaseSelect.Portal container={usePortalContainer()}>
          <BaseSelect.Positioner
            positionMethod="fixed"
            collisionBoundary={pageBoundary()}
            alignItemWithTrigger={false}
            align="start"
            sideOffset={6}
            collisionPadding={8}
            className="z-50 outline-none"
          >
            <BaseSelect.Popup data-select-popup="" className={cn(MENU_POPUP, "relative min-w-anchor overflow-hidden")} style={anchorWidthStyle(triggerWidth)}>
              <BaseSelect.ScrollUpArrow className={cn(SCROLL_ARROW, "top-0 rounded-t-menu")}>
                <Icon icon={icons.chevronUp} size="xs" />
              </BaseSelect.ScrollUpArrow>
              <BaseSelect.List className="-mx-6 flex min-h-0 flex-col overflow-y-auto overscroll-contain px-6 outline-none scrollbar-quiet">
                {groupsOf(options).map((g, gi) => (
                  <Fragment key={g.group ?? `g${gi}`}>
                    {gi > 0 && <div role="presentation" className={MENU_SEPARATOR} />}
                    <BaseSelect.Group className="flex flex-col">
                      {g.group && <BaseSelect.GroupLabel className={MENU_LABEL}>{g.group}</BaseSelect.GroupLabel>}
                      {g.items.map((o) => (
                        <BaseSelect.Item
                          key={o.value}
                          value={o.value}
                          {...(o.disabled ? { disabled: true } : {})}
                          label={o.textValue ?? (typeof o.label === "string" ? o.label : o.value)}
                          className={cn(MENU_ITEM, "group/opt")}
                        >
                          {multiple && (
                            <span className="pointer-events-none flex">
                              <OptionCheckbox />
                            </span>
                          )}
                          {o.icon && <Icon icon={o.icon} size="sm" className={cn(MENU_ICON, o.description && "self-start mt-10")} />}
                          <span className={cn("flex min-w-0 flex-1 flex-col", o.description ? "gap-2 py-8" : "py-6")}>
                            <BaseSelect.ItemText className="wrap-break-word whitespace-normal text-pretty">{o.label}</BaseSelect.ItemText>
                            {o.description && <span className="text-small font-regular whitespace-normal text-pretty text-fg-secondary">{o.description}</span>}
                          </span>
                          {!multiple && (
                            <BaseSelect.ItemIndicator
                              keepMounted
                              className={cn(
                                "flex size-16 shrink-0 items-center justify-center text-fg opacity-0 data-selected:opacity-100",
                                o.description && "self-start mt-10",
                              )}
                            >
                              <Icon icon={icons.check} size="sm" />
                            </BaseSelect.ItemIndicator>
                          )}
                        </BaseSelect.Item>
                      ))}
                    </BaseSelect.Group>
                  </Fragment>
                ))}
              </BaseSelect.List>
              <BaseSelect.ScrollDownArrow className={cn(SCROLL_ARROW, "bottom-0 rounded-b-menu")}>
                <Icon icon={icons.chevronDown} size="xs" />
              </BaseSelect.ScrollDownArrow>
            </BaseSelect.Popup>
          </BaseSelect.Positioner>
        </BaseSelect.Portal>
      </CSPProvider>
    </BaseSelect.Root>
  );
  if (field) return control;
  return withField({ label, description, error, optional, required, layout, id: fc.wrapId ?? id, disabled, className }, control);
});
Select.displayName = "Select";
