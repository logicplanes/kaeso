import { Tabs } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export type Scope = "active" | "archived" | "all";

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
    <Tabs variant="pill" value={value} onValueChange={(v) => onChange(v === "archived" || v === "all" ? v : "active")}>
      <Tabs.List aria-label={label}>
        <Tabs.Tab value="active">{activeLabel}</Tabs.Tab>
        {archivedLabel && <Tabs.Tab value="archived">{archivedLabel}</Tabs.Tab>}
        <Tabs.Tab value="all">{t("common.all", "All")}</Tabs.Tab>
      </Tabs.List>
    </Tabs>
  );
}
