import { Calendar, type CalendarProps } from "../calendar/calendar";
import { Popover } from "../popover/popover";
import { IsoField } from "./iso-field";
import { useUiText } from "../../i18n/context";

export interface DatePopupProps {
  date: Date | null;
  onDateChange: (date: Date | null) => void;
  onClose: () => void;
  weekStartsOn: CalendarProps["weekStartsOn"];
  disabledDays: ((date: Date) => boolean) | undefined;
}

export function DatePopup({ date, onDateChange, onClose, weekStartsOn, disabledDays }: DatePopupProps) {
  const t = useUiText();
  return (
    <Popover.Content padding="form" aria-label={t("datePicker.chooseDate", "Choose a date")}>
      <Calendar
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
        <IsoField label={t("datePicker.date", "Date")} value={date ?? undefined} onCommit={(d) => onDateChange(d)} />
      </div>
    </Popover.Content>
  );
}
