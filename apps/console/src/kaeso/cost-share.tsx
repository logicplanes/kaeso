import { Donut, Section, type Slice } from "@logicplanes/kaeso-ui";
import type { UsageRow } from "~/api/generated/types";
import { t } from "~/i18n";
import { chartState, periodUSD, sumUSD } from "./usage-panels";

export function CostShare({
  title,
  description,
  rows,
  label,
  loading,
  failed,
  onRetry,
}: {
  title: string;
  description: string;
  rows: UsageRow[];
  label: (key: string) => string;
  loading?: boolean | undefined;
  failed?: boolean | undefined;
  onRetry?: (() => void) | undefined;
}) {
  const slices: Slice[] = rows.filter((r) => Number(r.cost ?? 0) > 0).map((r) => ({ id: r.key, label: label(r.key), value: Number(r.cost) }));
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
