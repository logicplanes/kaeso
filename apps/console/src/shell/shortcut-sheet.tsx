import { Dialog, KbdSequence, Text } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

const sheet = (): { id: string; label: string; keys: string[] | string[][] }[] => [
  { id: "palette", label: t("shell.shortcut.palette", "Search and commands"), keys: ["mod", "k"] },
  { id: "create", label: t("shell.shortcut.create", "Create on this page"), keys: ["c"] },
  { id: "search", label: t("shell.shortcut.search", "Search this page"), keys: ["/"] },
  { id: "sidebar", label: t("shell.shortcut.sidebar", "Collapse or expand the sidebar"), keys: ["["] },
  { id: "close", label: t("shell.shortcut.close", "Close a dialog or panel"), keys: ["escape"] },
  { id: "overview", label: t("shell.goToOverview", "Go to the overview"), keys: [["g"], ["o"]] },
  { id: "keys", label: t("shell.shortcut.keys", "Go to API keys"), keys: [["g"], ["k"]] },
  { id: "models", label: t("shell.shortcut.models", "Go to models"), keys: [["g"], ["m"]] },
  { id: "logs", label: t("shell.shortcut.logs", "Go to request logs"), keys: [["g"], ["l"]] },
  { id: "project-settings", label: t("shell.shortcut.projectSettings", "Go to project settings"), keys: [["g"], ["p"]] },
  { id: "settings", label: t("shell.shortcut.settings", "Go to settings"), keys: [["g"], ["s"]] },
  { id: "audit-log", label: t("shell.shortcut.auditLog", "Go to the audit log"), keys: [["g"], ["a"]] },
  { id: "help", label: t("shell.shortcut.help", "This list"), keys: ["?"] },
];

export default function ShortcutSheet({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      size="sm"
      title={t("shell.shortcuts.title", "Keyboard shortcuts")}
      description={t("shell.shortcuts.description", "Single keys work when no field has focus.")}
    >
      <dl className="grid grid-cols-[1fr_auto] items-center gap-x-24 gap-y-12">
        {sheet().map((s) => (
          <div key={s.id} className="contents">
            <dt>
              <Text>{s.label}</Text>
            </dt>
            <dd>
              <KbdSequence keys={s.keys} />
            </dd>
          </div>
        ))}
      </dl>
    </Dialog>
  );
}
