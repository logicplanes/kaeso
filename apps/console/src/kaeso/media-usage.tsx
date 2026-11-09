import { Text } from "@logicplanes/kaeso-ui";
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

/** A usage group's cost: unknown when no request of it had a known price, and how many lacked one otherwise. */
export function UsageCost({ row }: { row: UsageRow }) {
  if (row.cost == null) return <Text tone="secondary">—</Text>;
  if (row.unpriced > 0 && row.unpriced >= row.requests && Number(row.cost) === 0)
    return <Text tone="secondary">{t("kaeso.usage.costUnknown", "Unknown")}</Text>;
  return (
    <span className="flex flex-col items-end">
      <Text tabular>{periodUSD(row.cost)}</Text>
      {row.unpriced > 0 && (
        <Text variant="caption" tone="tertiary">
          {t("kaeso.usage.unpriced", "{count, plural, one {# request} other {# requests}} unknown", { count: row.unpriced })}
        </Text>
      )}
    </span>
  );
}
