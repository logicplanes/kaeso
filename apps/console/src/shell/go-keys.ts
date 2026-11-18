import { useMemo } from "react";
import { useParams } from "@tanstack/react-router";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { readLast } from "./last";
import { orgPath, projectPath } from "./pages";
import { useProjectPages } from "./project-pages";
import { ORG_SETTINGS_PAGES } from "./settings-pages";

/** A page `g` and a letter opens from where the person is. */
export interface GoKey {
  id: string;
  key: string;
  label: string;
  to: string;
  /** Where the page belongs, for the shortcut sheet's groups. */
  scope: "project" | "org";
}

/**
 * The go-to keys that work here: a project's pages it lists with a `key` (inside a project and when the person may
 * open them), then the organization's overview outside a project, its audit log where no project page takes `a`, and
 * the settings. The shortcuts and the shortcut sheet both read this list, so they can't drift apart.
 */
export function useGoKeys(): GoKey[] {
  const params = useParams({ strict: false });
  const org = params.org ?? readLast()?.org;
  const project = params.project;
  const pages = useProjectPages(org ?? "", project ?? "");
  const orgRights = useOrgRights(org ?? "");
  return useMemo(() => {
    const keys: GoKey[] = [];
    if (org && project) {
      for (const p of pages) {
        keys.push({
          id: p.id,
          key: p.key,
          label: p.label(),
          to: p.path ? `${projectPath(org, project)}/${p.path}` : projectPath(org, project),
          scope: "project",
        });
      }
    }
    if (org && !project && orgRights.has("org.view")) {
      keys.push({ id: "org-overview", key: "o", label: t("shell.shortcut.orgOverview", "Organization overview"), to: orgPath(org), scope: "org" });
    }
    if (org && orgRights.has("audit_log.view") && !keys.some((k) => k.key === "a")) {
      keys.push({
        id: "org-audit-log",
        key: "a",
        label: t("shell.shortcut.orgAuditLog", "Organization audit log"),
        to: `/settings/orgs/${org}/audit-log`,
        scope: "org",
      });
    }
    const settings = org ? ORG_SETTINGS_PAGES.find((p) => !p.right || orgRights.has(p.right)) : undefined;
    keys.push(
      settings
        ? {
            id: "settings",
            key: "s",
            label: t("shell.orgSettings", "Organization settings"),
            to: `/settings/orgs/${org}/${settings.path}`,
            scope: "org",
          }
        : { id: "settings", key: "s", label: t("shell.shortcut.accountSettings", "Account settings"), to: "/settings/account/profile", scope: "org" },
    );
    return keys;
  }, [org, orgRights, pages, project]);
}
