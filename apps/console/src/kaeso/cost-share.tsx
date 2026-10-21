import { Donut, Section, seriesColorsNear, type Slice } from "@logicplanes/kaeso-ui";
import type { UsageRow } from "~/api/generated/types";
import { t } from "~/i18n";
import { chartState, periodUSD, sumUSD } from "./usage-panels";

export function CostShare({
  title,
  description,
  rows,
  label,
  hues,
  loading,
  failed,
  onRetry,
}: {
  title: string;
  description: string;
  rows: UsageRow[];
  label: (key: string) => string;
  hues?: ReadonlyMap<string, number> | undefined;
  loading?: boolean | undefined;
  failed?: boolean | undefined;
  onRetry?: (() => void) | undefined;
}) {
  const priced = rows.filter((r) => Number(r.cost ?? 0) > 0).sort((a, b) => Number(b.cost) - Number(a.cost));
  const colors = hues
    ? seriesColorsNear(
        priced.map((r) => r.key),
        hues,
      )
    : undefined;
  const slices: Slice[] = priced.map((r) => ({ id: r.key, label: label(r.key), value: Number(r.cost), color: colors?.get(r.key) }));
  const total = sumUSD(rows.map((r) => r.cost));
  return (
    <Section title={title} description={description}>
      <Donut
        aria-label={title}
        slices={slices}
        total={periodUSD(total)}
        totalLabel={t("kaeso.usage.totalCost", "Total cost")}
        formatValue={(v) => periodUSD(v.toFixed(6))}
        size="md"
        empty={slices.length === 0}
        emptyText={t("kaeso.usage.noCost", "No cost in this period.")}
        {...chartState({ loading, failed, onRetry })}
      />
    </Section>
  );
}
