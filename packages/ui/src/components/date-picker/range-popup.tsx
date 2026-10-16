import { useState, useSyncExternalStore } from "react";
import type { DateRange } from "react-day-picker";
import { Calendar, type CalendarProps } from "../calendar/calendar.js";
import { cn } from "../../utils/cn.js";
import { Popover } from "../popover/popover.js";
import { Button } from "../button/button.js";
import { MENU_ITEM } from "../menu/styles.js";
import { IsoField } from "./iso-field.js";
import type { DateRangePreset } from "./date-picker.js";
import { useUiText } from "../../i18n/context.js";

export interface RangePopupProps {
  range: DateRange | null;
  preset: string | null;
  presets: DateRangePreset[];
  onPick: (range: DateRange, preset: string | null) => void;
  onClose: () => void;
  weekStartsOn: CalendarProps["weekStartsOn"];
  disabledDays: ((date: Date) => boolean) | undefined;
}

const NARROW = "(max-width: 767px)";
function subscribeNarrow(onChange: () => void) {
  if (typeof window === "undefined" || !window.matchMedia) return () => {};
  const q = window.matchMedia(NARROW);
  q.addEventListener("change", onChange);
  return () => q.removeEventListener("change", onChange);
}
const isNarrow = () => typeof window !== "undefined" && !!window.matchMedia?.(NARROW).matches;
const useWide = () => !useSyncExternalStore(subscribeNarrow, isNarrow, () => false);

const monthOf = (d: Date) => new Date(d.getFullYear(), d.getMonth(), 1);
const addMonths = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth() + n, 1);
const sameDay = (a: Date | undefined, b: Date | undefined) => a?.toDateString() === b?.toDateString();

export function RangePopup({ range, preset, presets, onPick, onClose, weekStartsOn, disabledDays }: RangePopupProps) {
  const wide = useWide();
  const t = useUiText();
  const months = wide ? 2 : 1;
  const [draft, setDraft] = useState<DateRange | undefined>(range ?? undefined);
  const [custom, setCustom] = useState(preset === null);
  const [last, setLast] = useState(() => monthOf(range?.to ?? range?.from ?? new Date()));
  const first = addMonths(last, 1 - months);
  const shows = (d: Date) => d >= first && monthOf(d) <= last;
  const selected = custom ? null : preset;
  const edit = (next: DateRange | undefined) => {
    setDraft(next);
    setCustom(true);
  };
  const typed = (which: "from" | "to", d: Date) => {
    if (disabledDays?.(d)) return false;
    const other = which === "from" ? draft?.to : draft?.from;
    const next = which === "from" ? { from: d, to: other && other < d ? d : other } : { from: other && other > d ? d : other, to: d };
    edit(next);
    if (!shows(d)) setLast(which === "to" || months === 1 ? monthOf(d) : addMonths(monthOf(d), months - 1));
    return true;
  };
  const unchanged = sameDay(draft?.from, range?.from) && sameDay(draft?.to ?? draft?.from, range?.to);
  const item = cn(MENU_ITEM, "cursor-pointer hover:bg-fill-1 focus-ring-inset aria-selected:bg-fill-2", wide && "coarse:min-h-40");

  return (
    <Popover.Content padding="none" aria-label={t("datePicker.chooseRange", "Choose a date range")} className={cn(!wide && "w-360 max-w-(--available-width)")}>
      <div className={cn("flex", !wide && "flex-col")}>
        <div
          className={cn("shrink-0 gap-2 p-6", wide ? "flex min-w-160 flex-col hairline-r" : "grid grid-cols-2 hairline-b")}
          role="listbox"
          aria-label={t("datePicker.presets", "Presets")}
        >
          {presets.map((p) => {
            const id = p.id ?? p.label;
            return (
              <button key={id} type="button" role="option" aria-selected={selected === id} className={item} onClick={() => onPick(p.range(), id)}>
                {p.label}
              </button>
            );
          })}
          <button type="button" role="option" aria-selected={selected === null} className={item} onClick={() => setCustom(true)}>
            {t("datePicker.custom", "Custom")}
          </button>
        </div>
        <div className={cn("flex flex-col p-12", !wide && "items-center")}>
          <Calendar
            mode="range"
            numberOfMonths={months}
            selected={draft}
            month={first}
            onMonthChange={(m) => setLast(addMonths(m, months - 1))}
            onSelect={(r) => edit(r as DateRange | undefined)}
            weekStartsOn={weekStartsOn}
            disabled={disabledDays}
          />
          <div className="mt-12 flex items-center gap-8 self-stretch">
            <IsoField label={t("datePicker.startDate", "Start date")} value={draft?.from} onCommit={(d) => typed("from", d)} />
            <span className="text-fg-tertiary" aria-hidden>
              –
            </span>
            <IsoField label={t("datePicker.endDate", "End date")} value={draft?.to} onCommit={(d) => typed("to", d)} />
            <Button
              size="sm"
              variant="primary"
              disabled={!draft?.from}
              onClick={() => {
                if (!draft?.from) return;
                if (unchanged && !custom) onClose();
                else onPick({ from: draft.from, to: draft.to ?? draft.from }, null);
              }}
            >
              {t("datePicker.apply", "Apply")}
            </Button>
          </div>
        </div>
      </div>
    </Popover.Content>
  );
}
