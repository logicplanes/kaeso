import { SourceBadge, Text } from "@logicplanes/kaeso-ui";
import type { ExternalSourceView } from "~/api/generated/types";
import { t } from "~/i18n";

/** A source by its mark and name, or "A source" for one the person can't see. */
export function ReportedBy({ source }: { source: ExternalSourceView | undefined }) {
  return source ? (
    <SourceBadge kind={source.kind} name={source.name} size="sm" withName className="min-w-0" />
  ) : (
    <Text tone="secondary" truncate>
      {t("project.analytics.aSource", "A source")}
    </Text>
  );
}
