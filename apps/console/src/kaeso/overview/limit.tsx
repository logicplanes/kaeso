import { LimitMeter } from "@logicplanes/kaeso-ui";
import { formatNumber, formatRelative, formatUSD } from "@logicplanes/kaeso-ui/format";
import type { LimitView } from "~/api/generated/types";
import { alertMarks } from "~/format/alert-marks";
import { limitKindLabel, periodLabel } from "~/format/labels";
import { t } from "~/i18n";

export function Limit({ limit }: { limit: LimitView }) {
  const money = limit.kind === "spend_budget";
  const fmt = (v: string | number) => (money ? formatUSD(String(v)) : formatNumber(Number(v)));
  return (
    <LimitMeter
      kind={money ? "spend" : limit.kind === "request_rate" || limit.kind === "tool_call_rate" ? "requests" : "tokens"}
      scope={limitKindLabel(limit.kind)}
      window={periodLabel(limit.period)}
      used={Number(limit.used ?? 0)}
      limit={Number(limit.amount)}
      valueLabel={t("project.overview.limitUse", "{used} of {limit}", { used: fmt(limit.used ?? 0), limit: fmt(limit.amount) })}
      alerts={alertMarks(limit)}
      hard={limit.hard}
      resets={
        limit.period === "minute"
          ? t("project.overview.lastMinute", "Over the last minute")
          : limit.resets_at
            ? t("project.overview.resets", "Resets {when}", { when: formatRelative(limit.resets_at) })
            : t("project.overview.neverResets", "Never resets")
      }
    />
  );
}
