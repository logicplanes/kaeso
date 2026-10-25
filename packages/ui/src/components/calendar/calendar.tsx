import { createContext, forwardRef, use, useCallback, useMemo, type Ref } from "react";
import { DayPicker, type DateRange, type Formatters, type Labels, type RootProps } from "react-day-picker";
import { Icon } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";
import { useLocale, useUiText, type UiText } from "../../i18n/context.js";

export const KAESO_CALENDAR_VARIANTS = {
  mode: {
    single: { classes: "", description: "One date (default)" },
    range: { classes: "", description: "A range: a continuous strip between ink ends" },
  },
} as const;
export const KAESO_CALENDAR_DEFAULT_VARIANTS = { mode: "single" } as const;

const CAL_CLASSES = {
  root: "k-cal relative select-none",
  months: "flex gap-24",
  month: "flex flex-col gap-8",
  month_caption: "flex h-32 items-center px-8 text-body font-semibold text-fg",
  caption_label: "",
  nav: "absolute top-0 right-0 flex items-center gap-2",
  button_previous:
    "inline-flex size-32 items-center justify-center rounded-pill text-fg-secondary transition-control hover:bg-fill-1 hover:text-fg focus-ring disabled:opacity-40",
  button_next:
    "inline-flex size-32 items-center justify-center rounded-pill text-fg-secondary transition-control hover:bg-fill-1 hover:text-fg focus-ring disabled:opacity-40",
  month_grid: "border-collapse",
  weekdays: "",
  weekday: "h-32 w-36 text-caption font-medium text-fg-tertiary",
  weeks: "",
  week: "",
  day: "k-day",
  day_button: "",
  selected: "k-selected",
  today: "k-today",
  outside: "k-outside",
  disabled: "k-disabled",
  range_start: "k-range-start",
  range_middle: "k-range-middle",
  range_end: "k-range-end",
  hidden: "invisible",
  focused: "",
  chevron: "",
  footer: "",
};

function Chevron({ orientation }: { orientation?: "left" | "right" | "up" | "down" | undefined }) {
  return <Icon icon={orientation === "left" ? icons.chevronLeft : orientation === "right" ? icons.chevronRight : icons.chevronDown} size="sm" />;
}

const RootRef = createContext<Ref<HTMLDivElement> | null>(null);

function Root({ rootRef, ...props }: RootProps) {
  const ref = use(RootRef);
  const setRefs = useCallback(
    (el: HTMLDivElement | null) => {
      for (const r of [rootRef, ref]) {
        if (typeof r === "function") r(el);
        else if (r) r.current = el;
      }
    },
    [rootRef, ref],
  );
  return <div {...props} ref={setRefs} />;
}

function localized(locale: string, t: UiText): { formatters: Partial<Formatters>; labels: Partial<Labels>; lang: string } {
  const monthYear = new Intl.DateTimeFormat(locale, { month: "long", year: "numeric" });
  const weekdayShort = new Intl.DateTimeFormat(locale, { weekday: "short" });
  const weekdayLong = new Intl.DateTimeFormat(locale, { weekday: "long" });
  const full = new Intl.DateTimeFormat(locale, { dateStyle: "full" });
  return {
    lang: locale,
    formatters: {
      formatCaption: (month) => monthYear.format(month),
      formatWeekdayName: (weekday) => weekdayShort.format(weekday).replace(/\.$/, ""),
    },
    labels: {
      labelPrevious: () => t("calendar.previousMonth", "Go to the previous month"),
      labelNext: () => t("calendar.nextMonth", "Go to the next month"),
      labelNav: () => t("calendar.navigation", "Navigation bar"),
      labelGrid: (month) => monthYear.format(month),
      labelWeekday: (weekday) => weekdayLong.format(weekday),
      labelDayButton: (date, modifiers) => {
        let label = full.format(date);
        if (modifiers.today) label = t("calendar.today", "Today, {date}", { date: label });
        if (modifiers.selected) label = t("calendar.selected", "{date}, selected", { date: label });
        return label;
      },
    },
  };
}

export interface CalendarProps {
  mode?: keyof typeof KAESO_CALENDAR_VARIANTS.mode | undefined;
  selected?: Date | DateRange | undefined;
  onSelect?: ((value: Date | DateRange | undefined) => void) | undefined;
  month?: Date | undefined;
  defaultMonth?: Date | undefined;
  onMonthChange?: ((month: Date) => void) | undefined;
  numberOfMonths?: number | undefined;
  weekStartsOn?: 0 | 1 | 2 | 3 | 4 | 5 | 6 | undefined;
  disabled?: ((date: Date) => boolean) | undefined;
  className?: string | undefined;
}

export const Calendar = forwardRef<HTMLDivElement, CalendarProps>(function Calendar(
  { mode = "single", selected, onSelect, month, defaultMonth, onMonthChange, numberOfMonths = 1, weekStartsOn = 1, disabled, className },
  ref,
) {
  const locale = useLocale();
  const t = useUiText();
  const language = useMemo(() => {
    const own = localized(locale, t);
    return locale === "en" ? { labels: own.labels } : own;
  }, [locale, t]);
  const common = {
    ...language,
    classNames: numberOfMonths > 1 ? { ...CAL_CLASSES, outside: "k-outside invisible" } : CAL_CLASSES,
    components: { Chevron, Root },
    weekStartsOn,
    numberOfMonths,
    showOutsideDays: true,
    fixedWeeks: true,
    ...(month ? { month } : {}),
    ...(defaultMonth ? { defaultMonth } : {}),
    ...(onMonthChange ? { onMonthChange } : {}),
    ...(disabled ? { disabled } : {}),
    ...(className ? { className } : {}),
  };
  return (
    <RootRef value={ref}>
      {mode === "range" ? (
        <DayPicker mode="range" selected={selected as DateRange | undefined} onSelect={(r) => onSelect?.(r)} {...common} />
      ) : (
        <DayPicker mode="single" selected={selected as Date | undefined} onSelect={(d) => onSelect?.(d)} {...common} />
      )}
    </RootRef>
  );
});
Calendar.displayName = "Calendar";
