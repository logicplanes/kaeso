import { useCallback, useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate, useParams } from "@tanstack/react-router";
import { CommandPalette, icons, useTheme, type CommandGroup, type CommandItem } from "@logicplanes/kaeso-ui";
import { keysPageQuery } from "~/api/queries/keys";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { catalogueQuery } from "~/api/queries/models";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useMemberships, useOrgRights, useSession } from "~/auth/session";
import { t } from "~/i18n";
import { useCreateAction } from "./create-action";
import { readLast } from "./last";
import { ORG_AREA_PAGES, PROJECT_PAGES, orgPath, projectPath } from "./pages";
import { defaultProjectOf } from "./place";
import { readRecent, recentHint, type RecentIcons } from "./recent";
import { ACCOUNT_PAGES, INSTANCE_PAGES, ORG_SETTINGS_PAGES } from "./settings-pages";
import { openShortcutSheet } from "./shortcuts-store";
import { useProjectRights } from "~/auth/rights";

const orgKeywords = (id: string): string[] | undefined =>
  id === "overview"
    ? [t("shell.keyword.usage", "usage"), t("shell.keyword.cost", "cost"), t("shell.keyword.requests", "requests"), t("shell.keyword.tokens", "tokens")]
    : id === "providers"
      ? [t("shell.keyword.providerKeys", "provider keys"), t("shell.keyword.credentials", "credentials")]
      : undefined;

const RECENT_ICON: RecentIcons = {
  key: icons.key,
  model: icons.model,
  member: icons.member,
  "service-account": icons.serviceAccount,
  request: icons.requestLog,
};

export default function PaletteUI({
  open,
  onOpenChange,
  initialQuery = "",
  onReady,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialQuery?: string;
  onReady?: () => void;
}) {
  const [query, setQuery] = useState(initialQuery);
  const [openedWith, setOpenedWith] = useState(open);
  if (open !== openedWith) {
    setOpenedWith(open);
    setQuery(initialQuery);
  }
  useEffect(() => onReady?.(), [onReady]);
  const close = useCallback(() => onOpenChange(false), [onOpenChange]);
  const groups = usePaletteGroups(close, open, query.trim());
  return (
    <CommandPalette
      open={open}
      onOpenChange={onOpenChange}
      onQueryChange={setQuery}
      defaultQuery={initialQuery}
      hotkey={false}
      groups={groups}
      placeholder={t("shell.palette.placeholder", "Search pages, actions, keys, models and members")}
      emptyText={t("shell.palette.empty", "Nothing matches. Try another word.")}
    />
  );
}

function useFindGroup(org: string, project: string, query: string, enabled: boolean, close: () => void): CommandGroup | null {
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const on = enabled && !!project && query.length > 0;
  const keys = useQuery({ ...keysPageQuery(org, project, {}), enabled: on && rights.has("keys.all.view") });
  const models = useQuery({ ...catalogueQuery(org, project), enabled: on && rights.has("models.view") });
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: on && rights.has("project.members.view") });
  const accounts = useQuery({ ...serviceAccountsQuery(org, project), enabled: on && rights.has("service_accounts.view") });
  if (!on) return null;
  const q = query.toLowerCase();
  const has = (...texts: (string | null | undefined)[]) => texts.some((text) => text?.toLowerCase().includes(q));
  const base = projectPath(org, project);
  const go = (href: string) => () => {
    close();
    void navigate({ href });
  };
  const items: CommandItem[] = [
    ...(keys.data?.data ?? [])
      .filter((k) => has(k.name, k.redacted))
      .map((k) => ({
        id: `find:key:${k.id}`,
        label: k.name,
        icon: icons.key,
        hint: recentHint("key"),
        keywords: [query],
        onSelect: go(`${base}/keys?key=${k.id}`),
      })),
    ...(models.data ?? [])
      .filter((m) => has(m.model_id, m.effective.display_name))
      .map((m) => ({
        id: `find:model:${m.id}`,
        label: m.model_id,
        icon: icons.model,
        hint: recentHint("model"),
        keywords: [query],
        onSelect: go(`${base}/models?model=${m.id}`),
      })),
    ...(members.data ?? [])
      .filter((m) => has(m.user.name, m.user.email))
      .map((m) => ({
        id: `find:member:${m.user.id}`,
        label: m.user.name,
        icon: icons.member,
        hint: recentHint("member"),
        keywords: [query],
        onSelect: go(`${base}/members?member=${m.user.id}`),
      })),
    ...(accounts.data ?? [])
      .filter((a) => has(a.name))
      .map((a) => ({
        id: `find:sa:${a.id}`,
        label: a.name,
        icon: icons.serviceAccount,
        hint: recentHint("service-account"),
        keywords: [query],
        onSelect: go(`${base}/service-accounts?account=${a.id}`),
      })),
  ].slice(0, 20);
  return {
    label: t("shell.palette.find", "Find (in the first 100 of each list)"),
    items,
    loading: keys.isFetching || models.isFetching || members.isFetching || accounts.isFetching,
  };
}

function usePaletteGroups(close: () => void, open: boolean, query: string): CommandGroup[] {
  const navigate = useNavigate();
  const params = useParams({ strict: false });
  const session = useSession();
  const memberships = useMemberships();
  const last = readLast();
  const org = params.org ?? last?.org ?? memberships[0]?.org_id ?? "";
  const project = params.project ?? (org === last?.org ? last?.project : undefined) ?? defaultProjectOf(memberships, org) ?? "";
  const projectRights = useProjectRights(org, project);
  const orgRights = useOrgRights(org);
  const projects = useQuery({ ...orgProjectsQuery(org), enabled: !!org });
  const create = useCreateAction();
  const { setTheme } = useTheme();
  const find = useFindGroup(org, project, query, open, close);

  return useMemo(() => {
    const go = (to: string) => () => {
      close();
      void navigate({ to });
    };
    const goto: CommandItem[] = [
      ...PROJECT_PAGES.filter((p) => project && projectRights.has(p.right)).map((p) => ({
        id: `page:${p.id}`,
        label: p.label(),
        icon: p.icon,
        hint: t("common.project", "Project"),
        ...(p.shortcut ? { shortcut: p.shortcut } : {}),
        onSelect: go(p.path ? `${projectPath(org, project)}/${p.path}` : projectPath(org, project)),
      })),
      ...ORG_AREA_PAGES.filter((p) => org && orgRights.has(p.right)).map((p) => ({
        id: `org:${p.id}`,
        label: p.label(),
        icon: p.icon,
        hint: t("common.organization", "Organization"),
        ...(orgKeywords(p.id) ? { keywords: orgKeywords(p.id) } : {}),
        onSelect: go(p.path ? `${orgPath(org)}/${p.path}` : orgPath(org)),
      })),
      ...ORG_SETTINGS_PAGES.filter((p) => org && (!p.right || orgRights.has(p.right))).map((p) => ({
        id: `org-settings:${p.id}`,
        label: p.label(),
        icon: p.icon,
        hint: t("shell.orgSettings", "Organization settings"),
        onSelect: go(`/settings/orgs/${org}/${p.path}`),
      })),
      ...ACCOUNT_PAGES.map((p) => ({
        id: `account:${p.id}`,
        label: p.label(),
        icon: p.icon,
        hint: t("shell.account", "Account"),
        onSelect: go(`/settings/account/${p.path}`),
      })),
      ...(session.user.is_instance_admin
        ? INSTANCE_PAGES.map((p) => ({
            id: `instance:${p.id}`,
            label: p.label(),
            icon: p.icon,
            hint: t("shell.instance", "Instance"),
            onSelect: go(`/settings/instance/${p.path}`),
          }))
        : []),
    ];
    const groups: CommandGroup[] = [];
    const recent = open ? readRecent() : [];
    if (find) groups.push(find);
    if (recent.length)
      groups.push({
        label: t("shell.palette.recent", "Recent"),
        items: recent.map((r) => ({
          id: `recent:${r.kind}:${r.id}`,
          label: r.label,
          icon: RECENT_ICON[r.kind],
          hint: recentHint(r.kind),
          onSelect: go(r.href),
        })),
      });
    groups.push({ label: t("shell.palette.goTo", "Go to"), items: goto });
    const base = project ? projectPath(org, project) : "";
    const label = {
      key: t("shell.create.key", "Create key"),
      models: t("shell.create.models", "Add models"),
      alias: t("shell.create.alias", "Create alias"),
      serviceAccount: t("shell.create.serviceAccount", "Create service account"),
      limit: t("shell.create.limit", "Add limit"),
      project: t("shell.create.project", "Create project"),
      invite: t("shell.create.invite", "Invite member"),
    };
    const creates: CommandItem[] = [
      ...(create && !create.disabledReason
        ? [{ id: "create:page", label: create.label, icon: icons.add, shortcut: ["c"], onSelect: () => (close(), create.run()) }]
        : []),
      ...(base && projectRights.has("keys.own.create") && create?.label !== label.key
        ? [{ id: "create:key", label: label.key, icon: icons.key, onSelect: go(`${base}/keys?create=true`) }]
        : []),
      ...(base && projectRights.has("models.manage") && create?.label !== label.models
        ? [{ id: "create:model", label: label.models, icon: icons.model, onSelect: go(`${base}/models?add=true`) }]
        : []),
      ...(base && projectRights.has("models.manage") && create?.label !== label.alias
        ? [{ id: "create:alias", label: label.alias, icon: icons.alias, onSelect: go(`${base}/models?tab=aliases&add=true`) }]
        : []),
      ...(base && projectRights.has("service_accounts.create") && create?.label !== label.serviceAccount
        ? [{ id: "create:sa", label: label.serviceAccount, icon: icons.serviceAccount, onSelect: go(`${base}/service-accounts?create=true`) }]
        : []),
      ...(base && projectRights.has("limits.manage") && create?.label !== label.limit
        ? [{ id: "create:limit", label: label.limit, icon: icons.limit, onSelect: go(`${base}/limits?add=true`) }]
        : []),
      ...(org && orgRights.has("projects.create") && create?.label !== label.project
        ? [{ id: "create:project", label: label.project, icon: icons.project, onSelect: go(`${orgPath(org)}/projects?create=true`) }]
        : []),
      ...(org && orgRights.has("members.invite") && create?.label !== label.invite
        ? [{ id: "create:invite", label: label.invite, icon: icons.mail, onSelect: go(`/settings/orgs/${org}/members?tab=invites&invite=true`) }]
        : []),
    ];
    if (creates.length) groups.push({ label: t("common.create", "Create"), items: creates });
    const switchItems: CommandItem[] = [];
    if (memberships.length > 1) {
      switchItems.push({
        id: "switch:org",
        label: t("shell.palette.switchOrg", "Switch organization"),
        icon: icons.organization,
        children: [
          {
            label: t("shell.organizations", "Organizations"),
            items: memberships.map((m) => ({
              id: `switch:org:${m.org_id}`,
              label: m.org_name,
              icon: icons.organization,
              onSelect: go(orgPath(m.org_id)),
            })),
          },
        ],
      });
    }
    const projectList = projects.data ?? [];
    if (projectList.length > 1) {
      switchItems.push({
        id: "switch:project",
        label: t("shell.switchProject", "Switch project"),
        icon: icons.project,
        children: [
          {
            label: t("common.projects", "Projects"),
            items: projectList.map((p) => ({ id: `project:${p.id}`, label: p.name, icon: icons.project, onSelect: go(projectPath(org, p.id)) })),
          },
        ],
      });
    }
    if (switchItems.length) groups.push({ label: t("shell.palette.switch", "Switch"), items: switchItems });
    groups.push({
      label: t("shell.palette.theme", "Theme"),
      items: [
        {
          id: "theme:system",
          label: t("shell.palette.systemTheme", "Use the system theme"),
          icon: icons.themeSystem,
          onSelect: () => (close(), setTheme("system")),
        },
        {
          id: "theme:light",
          label: t("shell.palette.lightTheme", "Use the light theme"),
          icon: icons.themeLight,
          onSelect: () => (close(), setTheme("light")),
        },
        { id: "theme:dark", label: t("shell.palette.darkTheme", "Use the dark theme"), icon: icons.themeDark, onSelect: () => (close(), setTheme("dark")) },
      ],
    });
    groups.push({
      label: t("shell.palette.help", "Help"),
      items: [
        {
          id: "help:shortcuts",
          label: t("shell.shortcuts.title", "Keyboard shortcuts"),
          icon: icons.keyboard,
          shortcut: ["?"],
          onSelect: () => (close(), openShortcutSheet()),
        },
      ],
    });
    return groups;
  }, [close, create, find, memberships, navigate, open, org, orgRights, project, projectRights, projects.data, session.user.is_instance_admin, setTheme]);
}
