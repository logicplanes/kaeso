import { PillTabs } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export function ScopeTabs({
  all,
  onChange,
  label,
  activeLabel = t("common.active", "Active"),
}: {
  all: boolean;
  onChange: (all: boolean) => void;
  label: string;
  activeLabel?: string;
}) {
  return (
    <PillTabs value={all ? "all" : "active"} onValueChange={(v) => onChange(v === "all")}>
      <PillTabs.List aria-label={label}>
        <PillTabs.Tab value="all">{t("common.all", "All")}</PillTabs.Tab>
        <PillTabs.Tab value="active">{activeLabel}</PillTabs.Tab>
      </PillTabs.List>
    </PillTabs>
  );
}
