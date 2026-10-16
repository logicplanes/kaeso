import { Suspense, lazy, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "@tanstack/react-router";
import { useOrgRights } from "~/auth/session";
import { useCreateAction } from "./create-action";
import { takeEarlyKeys } from "./early-keys";
import { readLast } from "./last";
import { isPaletteOpen } from "./palette-store";
import { PROJECT_PAGES, orgPath, projectPath } from "./pages";
import { onShortcutSheet, openShortcutSheet } from "./shortcuts-store";
import { useProjectRights } from "~/auth/rights";

function typing(target: EventTarget | null): boolean {
  return target instanceof Element && !!target.closest("input, textarea, select, [contenteditable=true], [role=combobox]");
}

function layerOpen(): boolean {
  return !!document.querySelector("[role=dialog], [role=menu], [role=listbox]");
}

export function ShortcutsHost() {
  const navigate = useNavigate();
  const params = useParams({ strict: false });
  const create = useCreateAction();
  const org = params.org ?? readLast()?.org;
  const project = params.project;
  const rights = useProjectRights(org ?? "", project ?? "");
  const orgRights = useOrgRights(org ?? "");
  const pendingG = useRef<number>(0);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey || typing(e.target) || layerOpen() || isPaletteOpen()) return;
      if (Date.now() - pendingG.current < 1200) {
        pendingG.current = 0;
        const key = e.key;
        const page = PROJECT_PAGES.find((p) => p.key === key);
        if (page && org && project && rights.has(page.right)) {
          e.preventDefault();
          void navigate({ to: page.path ? `${projectPath(org, project)}/${page.path}` : projectPath(org, project) });
        } else if (key === "o" && org && !project && orgRights.has("org.view")) {
          e.preventDefault();
          void navigate({ to: orgPath(org) });
        } else if (e.key === "s" && org) {
          e.preventDefault();
          void navigate({ to: `/settings/orgs/${org}/general` });
        } else if (e.key === "a" && org) {
          e.preventDefault();
          void navigate({ to: `/settings/orgs/${org}/audit-log` });
        }
        return;
      }
      switch (e.key) {
        case "g":
          pendingG.current = Date.now();
          return;
        case "c":
          if (create && !create.disabledReason) {
            e.preventDefault();
            create.run();
          }
          return;
        case "/": {
          const search = document.querySelector<HTMLInputElement>("main input[type=search], [data-page-search] input");
          if (search) {
            e.preventDefault();
            search.focus();
          }
          return;
        }
        case "?":
          e.preventDefault();
          openShortcutSheet();
          return;
      }
    };
    window.addEventListener("keydown", onKey);
    for (const e of takeEarlyKeys()) onKey(e);
    return () => window.removeEventListener("keydown", onKey);
  }, [create, navigate, org, orgRights, project, rights]);

  return <ShortcutSheetHost />;
}

const ShortcutSheet = lazy(() => import("./shortcut-sheet"));

function ShortcutSheetHost() {
  const [open, setOpen] = useState(false);
  const [loaded, setLoaded] = useState(false);
  useEffect(
    () =>
      onShortcutSheet(() => {
        setLoaded(true);
        setOpen(true);
      }),
    [],
  );
  if (!loaded) return null;
  return (
    <Suspense fallback={null}>
      <ShortcutSheet open={open} onOpenChange={setOpen} />
    </Suspense>
  );
}
