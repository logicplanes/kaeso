import { useId, useRef, useState, useSyncExternalStore, type KeyboardEvent } from "react";
import type { DateRange } from "react-day-picker";
import { Calendar, type CalendarProps } from "../calendar/calendar.js";
import { cn } from "../../utils/cn.js";
import { Popover } from "../popover/popover.js";
import { Button } from "../button/button.js";
import { Icon } from "../icon/icon.js";
import { icons } from "../../icons/vocabulary.js";
import { ISO_PROBLEM, IsoField, isoProblemText, type IsoProblem } from "./iso-field.js";
import type { DateRangePreset } from "./date-picker.js";
import { useUiText } from "../../i18n/context.js";

function moveAmongOptions(e: KeyboardEvent<HTMLDivElement>, columns: number) {
  const options = [...e.currentTarget.querySelectorAll<HTMLElement>("[role=option]")];
  const at = options.indexOf(document.activeElement as HTMLElement);
  if (at < 0) return;
  const step: Record<string, number> = { ArrowDown: columns, ArrowUp: -columns, ...(columns > 1 ? { ArrowRight: 1, ArrowLeft: -1 } : {}) };
  const to = e.key === "Home" ? 0 : e.key === "End" ? options.length - 1 : step[e.key] !== undefined ? at + step[e.key]! : null;
  if (to === null) return;
  e.preventDefault();
  options[Math.max(0, Math.min(options.length - 1, to))]?.focus();
}

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
  const errorId = useId();
  const [problem, setProblem] = useState<{ field: "from" | "to"; problem: IsoProblem } | null>(null);
  const fields = useRef<HTMLDivElement | null>(null);
  const first = addMonths(last, 1 - months);
  const shows = (d: Date) => d >= first && monthOf(d) <= last;
  const selected = custom ? null : preset;
  const edit = (next: DateRange | undefined) => {
    setDraft(next);
    if (next?.from) setProblem((was) => (was?.problem === "missing" ? null : was));
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
  const item = cn(
    "flex min-h-34 w-full cursor-pointer items-center justify-between gap-10 rounded-item px-10 text-left text-body font-medium text-fg transition-control select-none coarse:min-h-44",
    "hover:bg-item-hover active:bg-item-pressed focus-ring-inset",
    wide && "coarse:min-h-40",
  );
  const check = (on: boolean) => <Icon icon={icons.check} size="sm" className={cn("shrink-0 text-fg", !on && "invisible")} />;
  const problemOf = (field: "from" | "to") => (problem?.field === field ? problem.problem : null);
  const report = (field: "from" | "to") => (p: IsoProblem) => setProblem(p ? { field, problem: p } : problem?.field === field ? null : problem);

  return (
    <Popover.Content padding="none" aria-label={t("datePicker.chooseRange", "Choose a date range")} className={cn(!wide && "w-360 max-w-(--available-width)")}>
      <div className={cn("flex", !wide && "flex-col")}>
        <div
          className={cn("shrink-0 gap-2 p-6", wide ? "flex min-w-160 flex-col hairline-r" : "grid grid-cols-2 hairline-b")}
          role="listbox"
          aria-label={t("datePicker.presets", "Presets")}
          aria-orientation={wide ? "vertical" : undefined}
          onKeyDown={(e) => moveAmongOptions(e, wide ? 1 : 2)}
        >
          {presets.map((p) => {
            const id = p.id ?? p.label;
            return (
              <button
                key={id}
                type="button"
                role="option"
                aria-selected={selected === id}
                tabIndex={selected === id ? 0 : -1}
                className={item}
                onClick={() => onPick(p.range(), id)}
              >
                <span className="min-w-0">{p.label}</span>
                {check(selected === id)}
              </button>
            );
          })}
          <button
            type="button"
            role="option"
            aria-selected={selected === null}
            tabIndex={selected === null || !presets.some((p) => (p.id ?? p.label) === selected) ? 0 : -1}
            className={item}
            onClick={() => setCustom(true)}
          >
            <span className="min-w-0">{t("datePicker.custom", "Custom")}</span>
            {check(selected === null)}
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
          <div ref={fields} className="mt-12 flex items-center gap-8 self-stretch">
            <IsoField
              label={t("datePicker.startDate", "Start date")}
              value={draft?.from}
              invalid={problemOf("from") !== null}
              errorId={errorId}
              onProblem={report("from")}
              onCommit={(d) => typed("from", d)}
            />
            <span className="text-fg-tertiary" aria-hidden>
              –
            </span>
            <IsoField
              label={t("datePicker.endDate", "End date")}
              value={draft?.to}
              invalid={problemOf("to") !== null}
              errorId={errorId}
              onProblem={report("to")}
              onCommit={(d) => typed("to", d)}
            />
            <Button
              size="sm"
              variant="primary"
              onClick={() => {
                if (!draft?.from) {
                  setProblem({ field: "from", problem: "missing" });
                  fields.current?.querySelector("input")?.focus();
                  return;
                }
                if (unchanged && !custom) onClose();
                else onPick({ from: draft.from, to: draft.to ?? draft.from }, null);
              }}
            >
              {t("datePicker.apply", "Apply")}
            </Button>
          </div>
          <p id={errorId} role="alert" className={cn(ISO_PROBLEM, "w-0 min-w-full")}>
            {problem ? isoProblemText(t, problem.problem) : ""}
          </p>
        </div>
      </div>
    </Popover.Content>
  );
}
