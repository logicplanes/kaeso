import { DateRangePicker } from "@logicplanes/kaeso-ui";
import type { PeriodId, PeriodSearch } from "~/format/period";
import { afterToday, pickedDay, pickedPeriod, periodPresets, pickerDate } from "~/format/picked-day";
import { t } from "~/i18n";

export function PeriodPicker({
  from,
  to,
  period,
  days,
  onChange,
}: {
  from: string;
  to: string;
  period: PeriodId | undefined;
  days: (id: PeriodId) => { from: string; to: string };
  onChange: (next: PeriodSearch) => void;
}) {
  const presets = periodPresets(days);
  return (
    <DateRangePicker
      size="sm"
      aria-label={t("kaeso.period.label", "Period")}
      value={{ from: pickerDate(from), to: pickerDate(to) }}
      preset={period ?? null}
      onValueChange={(r, picked) => {
        if (!r?.from || !r.to) return;
        const id = pickedPeriod(presets, picked.preset);
        onChange(id ? { period: id, from: undefined, to: undefined } : { period: undefined, from: pickedDay(r.from), to: pickedDay(r.to) });
      }}
      presets={presets}
      disabledDays={(d) => afterToday(d)}
    />
  );
}
