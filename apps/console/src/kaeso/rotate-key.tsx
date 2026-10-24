import { useState, type ComponentProps } from "react";
import { Radio, RadioCards, type StepDialog } from "@logicplanes/kaeso-ui";
import type { KeyRotate } from "~/api/generated/types";
import { t } from "~/i18n";
import { ExpiryPicker, defaultExpiry, expiryRequest, type Expiry } from "./expiry-picker";

type DialogStep = ComponentProps<typeof StepDialog>["steps"][number];

export type Grace = "0" | "1" | "24" | "168";

export interface RotationChoices {
  grace: Grace;
  setGrace: (g: Grace) => void;
  expiry: Expiry;
  setExpiry: (e: Expiry) => void;
  date: Date | null;
  setDate: (d: Date | null) => void;
  ready: boolean;
  body: KeyRotate;
}

export function useRotationChoices(maxDays: number | null | undefined): RotationChoices {
  const [grace, setGrace] = useState<Grace>("24");
  const [picked, setExpiry] = useState<Expiry | null>(null);
  const [date, setDate] = useState<Date | null>(null);
  const expiry = picked ?? defaultExpiry(maxDays);
  return {
    grace,
    setGrace,
    expiry,
    setExpiry,
    date,
    setDate,
    ready: expiry !== "date" || date !== null,
    body: { grace_hours: Number(grace), ...expiryRequest(expiry, date) },
  };
}

export function rotationSteps(c: RotationChoices, maxDays: number | null | undefined, scope: "org" | "project"): DialogStep[] {
  return [
    {
      id: "grace",
      title: t("kaeso.rotateKey.whenOldStops", "When does the old key stop working?"),
      description: t("kaeso.rotateKey.whenOldStopsHint", "Both keys work until then, so you can move your apps to the new one."),
      content: (
        <RadioCards aria-label={t("kaeso.rotateKey.gracePeriod", "Grace period")} value={c.grace} onValueChange={(v) => c.setGrace(v as Grace)}>
          <Radio
            value="0"
            label={t("kaeso.rotateKey.now", "Immediately")}
            description={t("kaeso.rotateKey.nowHint", "Apps still using the old key fail at once.")}
          />
          <Radio value="1" label={t("kaeso.rotateKey.afterHour", "After 1 hour")} />
          <Radio value="24" label={t("kaeso.rotateKey.afterDay", "After 24 hours")} />
          <Radio value="168" label={t("kaeso.rotateKey.afterWeek", "After 7 days")} />
        </RadioCards>
      ),
    },
    {
      id: "expiry",
      title: t("kaeso.rotateKey.whenNewExpires", "When does the new key expire?"),
      blockedReason: c.ready ? undefined : t("kaeso.createKey.pickDay", "Pick the day it expires"),
      content: <ExpiryPicker value={c.expiry} onChange={c.setExpiry} date={c.date} onDateChange={c.setDate} maxDays={maxDays} scope={scope} />,
    },
  ];
}

export function rotatedHint(grace: Grace): string {
  if (grace === "0") return t("kaeso.rotateKey.rotatedNow", "Copy the new key now: it's shown once, and the old key has stopped working.");
  if (grace === "1") return t("kaeso.rotateKey.rotatedHour", "Copy the new key now: it's shown once, and the old key stops in 1 hour.");
  if (grace === "24") return t("kaeso.rotateKey.rotatedDay", "Copy the new key now: it's shown once, and the old key stops in 24 hours.");
  return t("kaeso.rotateKey.rotatedWeek", "Copy the new key now: it's shown once, and the old key stops in 7 days.");
}
