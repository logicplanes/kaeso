import { Dialog, Kbd, Text } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";
import { useGoKeys, type GoKey } from "./go-keys";

interface Row {
  id: string;
  label: string;
  keys: string[] | string[][];
}

interface Group {
  id: string;
  title: string;
  rows: Row[];
}

const general = (): Row[] => [
  { id: "palette", label: t("shell.shortcut.palette", "Search and commands"), keys: ["mod", "k"] },
  { id: "create", label: t("shell.shortcut.create", "Create on this page"), keys: ["c"] },
  { id: "search", label: t("shell.shortcut.search", "Search this page"), keys: ["/"] },
  { id: "sidebar", label: t("shell.shortcut.sidebar", "Collapse or expand the sidebar"), keys: ["["] },
  { id: "close", label: t("shell.shortcut.close", "Close a dialog or panel"), keys: ["escape"] },
  { id: "help", label: t("shell.shortcut.help", "This list"), keys: ["?"] },
];

const onPages = (): Row[] => [
  { id: "next", label: t("shell.shortcut.next", "Next entry in request logs and tool calls"), keys: ["j"] },
  { id: "previous", label: t("shell.shortcut.previous", "Previous entry in request logs and tool calls"), keys: ["k"] },
  { id: "save-route", label: t("shell.shortcut.saveRoute", "Save a route"), keys: ["mod", "s"] },
  { id: "undo", label: t("shell.shortcut.undo", "Undo in the route editor"), keys: ["mod", "z"] },
  { id: "redo", label: t("shell.shortcut.redo", "Redo in the route editor"), keys: ["mod", "shift", "z"] },
];

const goRows = (keys: GoKey[]): Row[] => keys.map((k) => ({ id: `go:${k.id}`, label: k.label, keys: [["g"], [k.key]] }));

function groups(goKeys: GoKey[]): Group[] {
  const project = goKeys.filter((k) => k.scope === "project");
  return [
    { id: "general", title: t("shell.shortcuts.general", "Anywhere"), rows: general() },
    ...(project.length > 0 ? [{ id: "project", title: t("shell.shortcuts.project", "Go to a project page"), rows: goRows(project) }] : []),
    { id: "org", title: t("shell.shortcuts.go", "Go to"), rows: goRows(goKeys.filter((k) => k.scope === "org")) },
    { id: "pages", title: t("shell.shortcuts.pages", "On some pages"), rows: onPages() },
  ];
}

export default function ShortcutSheet({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const goKeys = useGoKeys();
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      size="sm"
      title={t("shell.shortcuts.title", "Keyboard shortcuts")}
      description={t("shell.shortcuts.description", "Single keys work when no field has focus.")}
    >
      <div className="flex flex-col gap-24">
        {groups(goKeys).map((g) => (
          <section key={g.id} aria-labelledby={`shortcuts-${g.id}`} className="flex flex-col gap-12">
            <Text as="h3" id={`shortcuts-${g.id}`} variant="small" weight="medium" tone="secondary">
              {g.title}
            </Text>
            <dl className="grid grid-cols-[1fr_auto] items-center gap-x-24 gap-y-12">
              {g.rows.map((s) => (
                <div key={s.id} className="contents">
                  <dt>
                    <Text>{s.label}</Text>
                  </dt>
                  <dd>
                    <Kbd keys={s.keys} />
                  </dd>
                </div>
              ))}
            </dl>
          </section>
        ))}
      </div>
    </Dialog>
  );
}
