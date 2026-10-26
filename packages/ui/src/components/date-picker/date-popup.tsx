import { Calendar, type CalendarProps } from "../calendar/calendar.js";
import { Popover } from "../popover/popover.js";
import { useId, useState } from "react";
import { ISO_PROBLEM, IsoField, isoProblemText, type IsoProblem } from "./iso-field.js";
import { useUiText } from "../../i18n/context.js";
import { useFocusDay } from "./focus-day.js";

export interface DatePopupProps {
  date: Date | null;
  onDateChange: (date: Date | null) => void;
  onClose: () => void;
  weekStartsOn: CalendarProps["weekStartsOn"];
  disabledDays: ((date: Date) => boolean) | undefined;
}

export function DatePopup({ date, onDateChange, onClose, weekStartsOn, disabledDays }: DatePopupProps) {
  const t = useUiText();
  const calendar = useFocusDay();
  const errorId = useId();
  const [problem, setProblem] = useState<IsoProblem>(null);
  return (
    <Popover.Content padding="form" aria-label={t("datePicker.chooseDate", "Choose a date")}>
      <Calendar
        ref={calendar}
        mode="single"
        selected={date ?? undefined}
        {...(date ? { defaultMonth: date } : {})}
        onSelect={(d) => {
          onDateChange((d as Date | undefined) ?? null);
          if (d) onClose();
        }}
        weekStartsOn={weekStartsOn}
        disabled={disabledDays}
      />
      <div className="mt-12 flex gap-8">
        <IsoField
          label={t("datePicker.date", "Date")}
          value={date ?? undefined}
          invalid={problem !== null}
          errorId={errorId}
          onProblem={setProblem}
          onCommit={(d) => {
            if (disabledDays?.(d)) return false;
            onDateChange(d);
            return true;
          }}
        />
      </div>
      <p id={errorId} role="alert" className={ISO_PROBLEM}>
        {isoProblemText(t, problem)}
      </p>
    </Popover.Content>
  );
}
