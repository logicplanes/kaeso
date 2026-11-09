import { Text, Tooltip } from "@logicplanes/kaeso-ui";
import { formatDuration, formatNumber } from "@logicplanes/kaeso-ui/format";
import type { UsageMedia, UsageRow } from "~/api/generated/types";
import { periodUSD } from "~/kaeso/usage-panels";
import { t } from "~/i18n";

/** What a usage group's media requests made or heard, such as "4 images · 2 s of audio", or "" for none. */
export function mediaSummary(m: UsageMedia | undefined): string {
  if (!m) return "";
  const parts: string[] = [];
  if (m.images) parts.push(t("kaeso.usage.media.images", "{count, plural, one {# image} other {# images}}", { count: m.images }));
  if (m.characters) parts.push(t("kaeso.usage.media.characters", "{count} characters", { count: formatNumber(m.characters) }));
  if (m.audio_ms) parts.push(t("kaeso.usage.media.audio", "{duration} of audio", { duration: formatDuration(m.audio_ms) }));
  if (m.videos)
    parts.push(
      m.video_ms
        ? t("kaeso.usage.media.videosLong", "{count, plural, one {# video} other {# videos}}, {duration}", {
            count: m.videos,
            duration: formatDuration(m.video_ms),
          })
        : t("kaeso.usage.media.videos", "{count, plural, one {# video} other {# videos}}", { count: m.videos }),
    );
  return parts.join(" · ");
}

export const hasMedia = (rows: readonly UsageRow[]) => rows.some((r) => mediaSummary(r.media) !== "");

export const costUnknown = (row: Pick<UsageRow, "cost" | "unpriced" | "requests">) =>
  row.unpriced > 0 && row.unpriced >= row.requests && Number(row.cost) === 0;

const marksUnpriced = (row: UsageRow) => row.cost != null && row.unpriced > 0 && !costUnknown(row);

const crowded = (row: UsageRow) => marksUnpriced(row) || (Number(row.cost ?? 0) > 0 && Number(row.cost) < 0.005);

// Table cells don't wrap: 106px fits "unter 0,01 $" and "+1 unbekannt" beside a phone's requests column.
export const costWidth = (rows: readonly UsageRow[], width: number, phone: boolean) => (rows.some(crowded) ? (phone ? 106 : 120) : width);

/** A usage group's cost: unknown when no request of it had a known price, and how many lacked one otherwise. */
export function UsageCost({ row }: { row: UsageRow }) {
  if (row.cost == null) return <Text tone="secondary">—</Text>;
  if (costUnknown(row)) return <Text tone="secondary">{t("kaeso.usage.costUnknown", "Unknown")}</Text>;
  if (!marksUnpriced(row)) return <Text tabular>{periodUSD(row.cost)}</Text>;
  const full = t("kaeso.usage.unpriced", "{count, plural, one {# request} other {# requests}} unknown", { count: row.unpriced });
  return (
    <span className="flex flex-col items-end">
      <Text tabular>{periodUSD(row.cost)}</Text>
      <Tooltip content={t("kaeso.usage.unpricedHint", "{requests}; the amount leaves them out.", { requests: full })}>
        <Text variant="caption" tone="tertiary" tabular tabIndex={0} aria-label={full} className="focus-ring">
          {t("kaeso.usage.unpricedShort", "+{count} unknown", { count: formatNumber(row.unpriced) })}
        </Text>
      </Tooltip>
    </span>
  );
}
