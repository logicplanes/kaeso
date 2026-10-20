import { PillTabs } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export type Scope = "active" | "archived" | "all";

export const SCOPES: readonly Scope[] = ["active", "archived", "all"];

export function ScopeTabs({
  value,
  onChange,
  label,
  activeLabel = t("common.active", "Active"),
  archivedLabel = t("common.archived", "Archived"),
}: {
  value: Scope;
  onChange: (scope: Scope) => void;
  label: string;
  activeLabel?: string;
  archivedLabel?: string | null;
}) {
  return (
    <PillTabs value={value} onValueChange={(v) => onChange(SCOPES.find((s) => s === v) ?? "active")}>
      <PillTabs.List aria-label={label}>
        <PillTabs.Tab value="active">{activeLabel}</PillTabs.Tab>
        {archivedLabel && <PillTabs.Tab value="archived">{archivedLabel}</PillTabs.Tab>}
        <PillTabs.Tab value="all">{t("common.all", "All")}</PillTabs.Tab>
      </PillTabs.List>
    </PillTabs>
  );
}
