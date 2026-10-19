import { Text, Tooltip } from "@logicplanes/kaeso-ui";
import { formatDate, formatRelative } from "@logicplanes/kaeso-ui/format";
import { t } from "~/i18n";

export function LastUsed({ at }: { at: string | null | undefined }) {
  if (!at) return <>{t("keys.neverUsed", "Never used")}</>;
  return (
    <Tooltip content={formatDate(at, { time: true })}>
      <Text variant="small" tone="secondary" tabular tabIndex={0}>
        {t("keys.used", "Used {when}", { when: formatRelative(at) })}
      </Text>
    </Tooltip>
  );
}
