import { forwardRef, useCallback, useEffect, useId, useMemo, useReducer, useRef, useState, type ButtonHTMLAttributes, type ReactNode } from "react";
import type { DateRange } from "react-day-picker";
import type { CalendarProps } from "../calendar/calendar.js";
import { cn } from "../../utils/cn.js";
import { useControllable } from "../../utils/use-controllable.js";
import { useField, useFieldControl, withField, type FieldWrapProps } from "../field/field.js";
import { Icon } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";
import { PopoverRoot, PopoverTrigger } from "../popover/popover-root.js";
import { formatDate, formatDateRange } from "../../format/index.js";
import { lazyModule, preload, type LazyModule } from "../../utils/lazy-module.js";
import { whenIdle } from "../../utils/when-idle.js";
import { englishText, useUiText, type UiText } from "../../i18n/context.js";
import { joinIds, useHiddenDescription } from "../../utils/hidden-description.js";

export const KAESO_DATE_PICKER_VARIANTS = {
  size: {
    sm: { classes: "h-32 pl-10 pr-12 gap-6 text-body", description: "32px" },
    md: { classes: "h-36 pl-14 pr-16 gap-8 text-body", description: "36px (default)" },
    lg: { classes: "h-40 pl-16 pr-18 gap-8 text-body", description: "40px" },
  },
} as const;
export const KAESO_DATE_PICKER_DEFAULT_VARIANTS = { size: "md" } as const;

const datePopup = lazyModule(() => import("./date-popup.js"), { base: () => import.meta.url });
const rangePopup = lazyModule(() => import("./range-popup.js"), { base: () => import.meta.url });
const preloadDatePopup = () => preload(datePopup);
const preloadRangePopup = () => preload(rangePopup);

interface LoadedOpen {
  open: boolean;
  pending: boolean;
  setOpen: (open: boolean) => void;
}

function useLoadedOpen(popup: LazyModule<unknown>): LoadedOpen {
  const [wanted, setWanted] = useState(false);
  const [, arrived] = useReducer((n: number) => n + 1, 0);
  const loaded = !!popup.current();
  const pending = wanted && !loaded;
  useEffect(() => {
    if (!pending) return;
    let live = true;
    popup.load().then(
      () => live && arrived(),
      () => live && setWanted(false),
    );
    return () => {
      live = false;
    };
  }, [pending, popup]);
  const setOpen = useCallback((next: boolean) => setWanted((was) => (next && was && !popup.current() ? false : next)), [popup]);
  return { open: wanted && loaded, pending, setOpen };
}

interface TriggerButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  size: keyof typeof KAESO_DATE_PICKER_VARIANTS.size;
  children: ReactNode;
  placeholder: boolean;
  invalid?: boolean | undefined;
  controlProps?: object | undefined;
  force?: string | undefined;
  preload: () => void;
  pending: boolean;
  onCancel: () => void;
  labelled: boolean;
  "aria-label"?: string | undefined;
  "aria-labelledby"?: string | undefined;
}

const TriggerButton = forwardRef<HTMLButtonElement, TriggerButtonProps>(function TriggerButton(
  {
    size,
    children,
    placeholder,
    invalid,
    disabled,
    id,
    controlProps,
    force,
    preload,
    pending,
    onCancel,
    labelled,
    "aria-label": ariaLabel,
    "aria-labelledby": labelledBy,
    className,
    ...rest
  },
  ref,
) {
  const valueId = useId();
  const purpose = useHiddenDescription(labelledBy ? undefined : ariaLabel);
  const self = useRef<HTMLButtonElement | null>(null);
  const setRef = useCallback(
    (el: HTMLButtonElement | null) => {
      self.current = el;
      if (typeof ref === "function") ref(el);
      else if (ref) ref.current = el;
    },
    [ref],
  );
  useEffect(() => whenIdle(preload), [preload]);
  useEffect(() => {
    if (!pending) return;
    const doc = self.current?.ownerDocument ?? document;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onCancel();
    };
    const onPointerDown = (e: PointerEvent) => {
      if (!self.current?.contains(e.target as Node)) onCancel();
    };
    doc.addEventListener("keydown", onKeyDown, true);
    doc.addEventListener("pointerdown", onPointerDown, true);
    return () => {
      doc.removeEventListener("keydown", onKeyDown, true);
      doc.removeEventListener("pointerdown", onPointerDown, true);
    };
  }, [pending, onCancel]);
  const control = (controlProps ?? {}) as { "aria-describedby"?: string | undefined };
  const describedBy = joinIds(purpose.id, labelled && !labelledBy && valueId, control["aria-describedby"]);
  return (
    <>
      <button
        ref={setRef}
        type="button"
        {...rest}
        {...(pending ? { "aria-expanded": true, "data-popup-open": "" } : {})}
        onPointerEnter={(e) => {
          preload();
          rest.onPointerEnter?.(e);
        }}
        onFocus={(e) => {
          preload();
          rest.onFocus?.(e);
        }}
        onBlur={(e) => {
          if (pending && e.relatedTarget && !e.currentTarget.contains(e.relatedTarget)) onCancel();
          rest.onBlur?.(e);
        }}
        disabled={disabled}
        {...control}
        {...(labelledBy ? { "aria-labelledby": `${labelledBy} ${valueId}` } : {})}
        {...(describedBy ? { "aria-describedby": describedBy } : {})}
        {...(id ? { id } : {})}
        {...(invalid ? { "aria-invalid": true } : {})}
        {...(force ? { "data-force": force } : {})}
        className={cn(
          "inline-flex min-w-0 cursor-pointer items-center rounded-pill font-medium whitespace-nowrap material-secondary transition-control focus-ring disabled:cursor-not-allowed",
          KAESO_DATE_PICKER_VARIANTS.size[size].classes,
          invalid && "ring-invalid",
          className,
        )}
      >
        <Icon icon={icons.calendar} size="sm" className="text-fg-secondary" />
        <span id={valueId} className={cn("truncate tabular", placeholder && "font-regular text-fg-tertiary")}>
          {children}
        </span>
      </button>
      {purpose.node}
    </>
  );
});

export interface DatePickerProps extends FieldWrapProps {
  value?: Date | null | undefined;
  defaultValue?: Date | null | undefined;
  onValueChange?: ((value: Date | null) => void) | undefined;
  placeholder?: string | undefined;
  size?: keyof typeof KAESO_DATE_PICKER_VARIANTS.size | undefined;
  weekStartsOn?: CalendarProps["weekStartsOn"] | undefined;
  disabledDays?: ((date: Date) => boolean) | undefined;
  disabled?: boolean | undefined;
  id?: string | undefined;
  className?: string | undefined;
  "aria-label"?: string | undefined;
  "aria-labelledby"?: string | undefined;
  "data-force"?: string | undefined;
}

export const DatePicker = forwardRef<HTMLDivElement, DatePickerProps>(function DatePicker(props, ref) {
  const {
    value,
    defaultValue = null,
    onValueChange,
    placeholder,
    size = "md",
    weekStartsOn = 1,
    disabledDays,
    disabled,
    id,
    className,
    label,
    description,
    error,
    optional,
    required,
    layout,
    "aria-label": ariaLabel,
    "aria-labelledby": ariaLabelledBy,
    "data-force": force,
  } = props;
  const field = useField();
  const fc = useFieldControl(props);
  const t = useUiText();
  const naming = { ...(ariaLabel ? { "aria-label": ariaLabel } : {}), ...(ariaLabelledBy ? { "aria-labelledby": ariaLabelledBy } : {}) };
  const labelled = !!(label || field);
  const [date, setDate] = useControllable<Date | null>(value, defaultValue, onValueChange);
  const { open, pending, setOpen } = useLoadedOpen(datePopup);
  const cancel = useCallback(() => setOpen(false), [setOpen]);
  const DatePopup = datePopup.current()?.DatePopup;
  const control = (
    <div ref={ref} className={cn("inline-flex", !(label || description || error) && className)}>
      <PopoverRoot open={open} onOpenChange={setOpen}>
        <PopoverTrigger>
          <TriggerButton
            size={size}
            placeholder={!date}
            invalid={!!error}
            disabled={disabled}
            id={id}
            controlProps={fc.controlProps}
            force={force}
            preload={preloadDatePopup}
            pending={pending}
            onCancel={cancel}
            labelled={labelled}
            {...naming}
          >
            {date ? formatDate(date) : (placeholder ?? t("datePicker.pickDate", "Pick a date"))}
          </TriggerButton>
        </PopoverTrigger>
        {DatePopup && <DatePopup date={date} onDateChange={setDate} onClose={() => setOpen(false)} weekStartsOn={weekStartsOn} disabledDays={disabledDays} />}
      </PopoverRoot>
    </div>
  );
  if (field) return control;
  return withField({ label, description, error, optional, required, layout, id: fc.wrapId ?? id, disabled, className }, control);
});
DatePicker.displayName = "DatePicker";

export interface DateRangePreset {
  id?: string | undefined;
  label: string;
  range: () => DateRange;
}

const day = 86_400_000;
const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
function rangePresets(t: UiText): DateRangePreset[] {
  return [
    { id: "Last 24 hours", label: t("datePicker.last24Hours", "Last 24 hours"), range: () => ({ from: new Date(Date.now() - day), to: new Date() }) },
    { id: "Last 7 days", label: t("datePicker.last7Days", "Last 7 days"), range: () => ({ from: startOfDay(new Date(Date.now() - 6 * day)), to: new Date() }) },
    {
      id: "Last 30 days",
      label: t("datePicker.last30Days", "Last 30 days"),
      range: () => ({ from: startOfDay(new Date(Date.now() - 29 * day)), to: new Date() }),
    },
    {
      id: "This month",
      label: t("datePicker.thisMonth", "This month"),
      range: () => ({ from: new Date(new Date().getFullYear(), new Date().getMonth(), 1), to: new Date() }),
    },
    {
      id: "Last month",
      label: t("datePicker.lastMonth", "Last month"),
      range: () => {
        const n = new Date();
        return { from: new Date(n.getFullYear(), n.getMonth() - 1, 1), to: new Date(n.getFullYear(), n.getMonth(), 0) };
      },
    },
  ];
}
export const DEFAULT_RANGE_PRESETS: DateRangePreset[] = rangePresets(englishText);
const presetId = (p: DateRangePreset) => p.id ?? p.label;
const presetIdOf = (p: DateRangePreset | undefined) => (p ? presetId(p) : null);

export interface DateRangePickerProps extends Omit<DatePickerProps, "value" | "defaultValue" | "onValueChange"> {
  value?: DateRange | null | undefined;
  defaultValue?: DateRange | null | undefined;
  onValueChange?: ((value: DateRange | null, details: { preset: string | null }) => void) | undefined;
  presets?: DateRangePreset[] | undefined;
  preset?: string | null | undefined;
}

const rangeDays = (r: DateRange | null | undefined) => `${r?.from?.toDateString()}/${r?.to?.toDateString()}`;

export const DateRangePicker = forwardRef<HTMLDivElement, DateRangePickerProps>(function DateRangePicker(props, ref) {
  const {
    value,
    defaultValue = null,
    onValueChange,
    placeholder,
    size = "md",
    weekStartsOn = 1,
    disabledDays,
    disabled,
    id,
    className,
    presets: presetsProp,
    preset: presetProp,
    label,
    description,
    error,
    optional,
    required,
    layout,
    "aria-label": ariaLabel,
    "aria-labelledby": ariaLabelledBy,
    "data-force": force,
  } = props;
  const field = useField();
  const fc = useFieldControl(props);
  const t = useUiText();
  const defaults = useMemo(() => rangePresets(t), [t]);
  const presets = presetsProp ?? defaults;
  const naming = { ...(ariaLabel ? { "aria-label": ariaLabel } : {}), ...(ariaLabelledBy ? { "aria-labelledby": ariaLabelledBy } : {}) };
  const labelled = !!(label || field);
  const [inner, setInner] = useState<DateRange | null>(defaultValue);
  const range = value !== undefined ? value : inner;
  const [picked, setPicked] = useState<string | null>(null);
  const days = rangeDays(range);
  const preset = presetProp !== undefined ? presetProp : presetIdOf(presets.find((p) => presetId(p) === picked && rangeDays(p.range()) === days));
  const { open, pending, setOpen } = useLoadedOpen(rangePopup);
  const cancel = useCallback(() => setOpen(false), [setOpen]);
  const [opening, setOpening] = useState(0);
  const openRange = useCallback(
    (next: boolean) => {
      if (next && !open) setOpening((n) => n + 1);
      setOpen(next);
    },
    [open, setOpen],
  );
  const RangePopup = rangePopup.current()?.RangePopup;
  const presetName = preset === null ? null : (presets.find((p) => presetId(p) === preset)?.label ?? preset);
  const display =
    presetName ??
    (range?.from && range.to
      ? formatDateRange(range.from, range.to)
      : range?.from
        ? formatDate(range.from)
        : (placeholder ?? t("datePicker.pickRange", "Pick a range")));
  const control = (
    <div ref={ref} className={cn("inline-flex", !(label || description || error) && className)}>
      <PopoverRoot open={open} onOpenChange={openRange}>
        <PopoverTrigger>
          <TriggerButton
            size={size}
            placeholder={!range?.from && !preset}
            invalid={!!error}
            disabled={disabled}
            id={id}
            controlProps={fc.controlProps}
            force={force}
            preload={preloadRangePopup}
            pending={pending}
            onCancel={cancel}
            labelled={labelled}
            {...naming}
          >
            {display}
          </TriggerButton>
        </PopoverTrigger>
        {RangePopup && (
          <RangePopup
            key={opening}
            range={range}
            preset={preset}
            presets={presets}
            onPick={(next, from) => {
              if (value === undefined) setInner(next);
              setPicked(from);
              onValueChange?.(next, { preset: from });
              setOpen(false);
            }}
            onClose={() => setOpen(false)}
            weekStartsOn={weekStartsOn}
            disabledDays={disabledDays}
          />
        )}
      </PopoverRoot>
    </div>
  );
  if (field) return control;
  return withField({ label, description, error, optional, required, layout, id: fc.wrapId ?? id, disabled, className }, control);
});
DateRangePicker.displayName = "DateRangePicker";

export type { DateRange };
