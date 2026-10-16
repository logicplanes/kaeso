import { useState } from "react";
import { DatePicker, Select } from "@logicplanes/kaeso-ui";
import type { ExpiryRequest } from "~/api/generated/types";
import { pickedDay } from "~/format/picked-day";
import { t } from "~/i18n";

export type Expiry = "1" | "7" | "30" | "90" | "date" | "never";

export function defaultExpiry(maxDays: number | null | undefined): Expiry {
  if (!maxDays || maxDays >= 90) return "90";
  return (["30", "7", "1"] as const).find((d) => Number(d) <= maxDays) ?? "date";
}

export function expiryRequest(choice: Expiry, date: Date | null): ExpiryRequest {
  if (choice === "never") return {};
  if (choice === "date") return date ? { expires_on: pickedDay(date) } : {};
  return { expires_in_days: Number(choice) };
}

export function ExpiryPicker({
  value,
  onChange,
  date,
  onDateChange,
  maxDays,
  scope = "project",
}: {
  value: Expiry;
  onChange: (v: Expiry) => void;
  date: Date | null;
  onDateChange: (d: Date | null) => void;
  maxDays: number | null | undefined;
  scope?: string;
}) {
  const [now] = useState(() => Date.now());
  const inDays = (count: number) => t("kaeso.expiry.inDays", "{count, plural, one {In # day} other {In # days}}", { count });
  const options = [
    { value: "1", label: inDays(1) },
    { value: "7", label: inDays(7) },
    { value: "30", label: inDays(30) },
    { value: "90", label: inDays(90) },
    { value: "date", label: t("kaeso.expiry.onDate", "On a date") },
    { value: "never", label: t("common.never", "Never") },
  ].filter((o) => {
    if (!maxDays) return true;
    if (o.value === "never") return false;
    return o.value === "date" || Number(o.value) <= maxDays;
  });
  const latest = maxDays ? now + maxDays * 86_400_000 : null;
  return (
    <div className="flex flex-col gap-12">
      <Select
        label={t("common.expires", "Expires")}
        options={options}
        value={value}
        onValueChange={(v) => onChange(v as Expiry)}
        fullWidth
        description={
          !maxDays
            ? t("kaeso.expiry.hint", "Choose the shortest time the key is needed.")
            : scope === "org" || scope === "This organization"
              ? t(
                  "kaeso.expiry.orgMax",
                  "{count, plural, one {This organization allows keys for up to # day.} other {This organization allows keys for up to # days.}}",
                  {
                    count: maxDays,
                  },
                )
              : t(
                  "kaeso.expiry.projectMax",
                  "{count, plural, one {This project allows keys for up to # day.} other {This project allows keys for up to # days.}}",
                  {
                    count: maxDays,
                  },
                )
        }
      />
      {value === "date" && (
        <DatePicker
          label={t("kaeso.expiry.date", "Expiry date")}
          value={date}
          onValueChange={onDateChange}
          disabledDays={(d) => d.getTime() <= now || (latest !== null && d.getTime() > latest)}
          description={t("kaeso.expiry.dateHint", "The key stops working at the start of this day (UTC).")}
        />
      )}
    </div>
  );
}
