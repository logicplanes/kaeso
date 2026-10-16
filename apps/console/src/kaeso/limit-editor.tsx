import { Field, NumberInput, Segmented, Select, Switch, TagInput, Text } from "@logicplanes/kaeso-ui";
import type { CatalogueModelView } from "~/api/generated/types";
import { modelOption } from "~/format/models";
import { limitKindLabel, limitUnit } from "~/format/labels";
import { t } from "~/i18n";

export type LimitKind = "request_rate" | "token_rate" | "token_budget" | "spend_budget";
type BudgetPeriod = "day" | "month" | "total";

export interface LimitDraft {
  kind: LimitKind;
  amount: string;
  period: "minute" | BudgetPeriod;
  model: string | null;
  hard: boolean;
  alert_thresholds: number[];
}

export const newLimit = (): LimitDraft => ({ kind: "spend_budget", amount: "", period: "month", model: null, hard: true, alert_thresholds: [50, 80, 100] });

const isRate = (k: LimitKind) => k === "request_rate" || k === "token_rate";

export const alertThresholds = (d: LimitDraft): number[] => (isRate(d.kind) ? [] : d.alert_thresholds);

const KINDS: LimitKind[] = ["spend_budget", "token_budget", "request_rate", "token_rate"];
const kindHelp = (k: LimitKind): string => {
  switch (k) {
    case "spend_budget":
      return t("kaeso.limitEditor.spendHelp", "US dollars per day, month or in total");
    case "token_budget":
      return t("kaeso.limitEditor.tokenBudgetHelp", "Tokens per day, month or in total");
    case "request_rate":
      return t("kaeso.limitEditor.requestRateHelp", "Requests per minute");
    default:
      return t("kaeso.limitEditor.tokenRateHelp", "Tokens per minute");
  }
};

const SPEND_CAP = 1_000_000_000n;

function exceedsSpendCap(amount: string): boolean {
  const digits = /^(\d+)(?:\.(\d*))?$/.exec(amount.trim());
  if (!digits) return Number(amount) > Number(SPEND_CAP);
  const [, whole = "0", fraction = ""] = digits;
  const dollars = BigInt(whole);
  return dollars > SPEND_CAP || (dollars === SPEND_CAP && /[1-9]/.test(fraction));
}

export function limitProblem(d: LimitDraft): string | null {
  const amount = Number(d.amount);
  if (!d.amount || !Number.isFinite(amount) || amount <= 0) return t("kaeso.limitEditor.amountAboveZero", "Enter an amount above zero.");
  if (d.kind === "spend_budget" && exceedsSpendCap(d.amount)) return t("kaeso.limitEditor.spendRange", "Enter an amount up to 1,000,000,000 USD.");
  if (d.kind !== "spend_budget" && !Number.isInteger(amount)) return t("kaeso.limitEditor.wholeNumbers", "Requests and tokens are whole numbers.");
  if (alertThresholds(d).some((p) => p < 1 || p > 100)) return t("kaeso.limitEditor.thresholdRange", "Alert thresholds are between 1 and 100 percent.");
  if (alertThresholds(d).length > 10) return t("kaeso.limitEditor.thresholdCount", "Set up to 10 alert thresholds.");
  return null;
}

export function limitBody(d: LimitDraft) {
  return {
    kind: d.kind,
    amount: d.amount,
    period: isRate(d.kind) ? "minute" : d.period,
    ...(d.model ? { model: d.model } : {}),
    hard: isRate(d.kind) ? true : d.hard,
    alert_thresholds: alertThresholds(d),
  };
}

export function LimitEditor({
  value,
  onChange,
  models,
  fixedKind = false,
}: {
  value: LimitDraft;
  onChange: (d: LimitDraft) => void;
  models: CatalogueModelView[];
  fixedKind?: boolean;
}) {
  const set = (patch: Partial<LimitDraft>) => onChange({ ...value, ...patch });
  const rate = isRate(value.kind);
  return (
    <div className="flex flex-col gap-16">
      {!fixedKind && (
        <Select
          label={t("kaeso.limitEditor.counts", "Counts")}
          fullWidth
          value={value.kind}
          onValueChange={(k) => set({ kind: k as LimitKind, period: isRate(k as LimitKind) ? "minute" : value.period === "minute" ? "month" : value.period })}
          options={KINDS.map((k) => ({ value: k, label: limitKindLabel(k), description: kindHelp(k) }))}
        />
      )}
      <NumberInput
        label={t("kaeso.limitEditor.amount", "Amount")}
        value={value.amount}
        onValueChange={(amount) => set({ amount })}
        min="0"
        step={value.kind === "spend_budget" ? "0.01" : "1"}
        unit={limitUnit(value.kind)}
        {...(value.kind === "spend_budget" ? { prefix: "$" } : {})}
      />
      {rate ? (
        <Text variant="small" tone="secondary">
          {t("kaeso.limitEditor.rateHint", "Rates count over a sliding minute and always refuse requests above them.")}
        </Text>
      ) : (
        !fixedKind && (
          <Field
            label={t("kaeso.limitEditor.period", "Period")}
            description={t("kaeso.limitEditor.periodHint", "UTC calendar days and months; Total never resets.")}
          >
            <Segmented
              aria-label={t("kaeso.limitEditor.period", "Period")}
              value={value.period}
              onValueChange={(p) => set({ period: p as BudgetPeriod })}
              options={[
                { value: "day", label: t("kaeso.limitEditor.day", "Day") },
                { value: "month", label: t("kaeso.limitEditor.month", "Month") },
                { value: "total", label: t("common.total", "Total") },
              ]}
              fullWidth
            />
          </Field>
        )
      )}
      {!fixedKind && (
        <Select
          label={t("common.model", "Model")}
          fullWidth
          value={value.model ?? "all"}
          onValueChange={(m) => set({ model: m === "all" ? null : m })}
          options={[{ value: "all", label: t("kaeso.limits.allModels", "All models") }, ...models.map(modelOption)]}
        />
      )}
      {!rate && (
        <Switch
          label={t("kaeso.limitEditor.hard", "Refuse requests when used up")}
          description={
            value.hard
              ? t("kaeso.limitEditor.hardHint", "A hard budget: requests are refused once it is used up.")
              : t("kaeso.limitEditor.softHint", "Alerts only: requests continue past it.")
          }
          checked={value.hard}
          onCheckedChange={(hard) => set({ hard })}
        />
      )}
      {!rate && (
        <TagInput
          label={t("kaeso.limitEditor.alertAt", "Alert at")}
          description={t("kaeso.limitEditor.alertAtHint", "Percent of the amount; owners, admins and key owners get an email at each.")}
          value={value.alert_thresholds.map(String)}
          onValueChange={(tags) =>
            set({
              alert_thresholds: [...new Set(tags.map(Number).filter((n) => Number.isInteger(n)))].sort((a, b) => a - b),
            })
          }
          validate={(v) =>
            /^\d+$/.test(v) && Number(v) >= 1 && Number(v) <= 100 ? true : t("kaeso.limitEditor.wholePercent", "A whole percent between 1 and 100")
          }
        />
      )}
    </div>
  );
}
