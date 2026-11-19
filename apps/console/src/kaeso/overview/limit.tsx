import { LimitMeter } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import type { LimitView } from "~/api/generated/types";
import { alertMarks } from "~/format/alert-marks";
import { limitUse } from "~/format/limit-use";
import { limitKindLabel, periodLabel } from "~/format/labels";
import { t } from "~/i18n";
import { LimitFigure } from "../limit-figure";

export function Limit({ limit }: { limit: LimitView }) {
  const money = limit.kind === "spend_budget";
  return (
    <LimitMeter
      kind={money ? "spend" : limit.kind === "request_rate" || limit.kind === "tool_call_rate" ? "requests" : "tokens"}
      scope={limitKindLabel(limit.kind)}
      window={periodLabel(limit.period)}
      used={Number(limit.used ?? 0)}
      limit={Number(limit.amount)}
      valueLabel={<LimitFigure figure={limitUse(limit, ({ used, amount }) => t("project.overview.limitUse", "{used} of {limit}", { used, limit: amount }))} />}
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
