import { useState, type SyntheticEvent } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate, useParams } from "@tanstack/react-router";
import { SearchInput, Sidebar, Text } from "@logicplanes/kaeso-ui";
import { prefetchSettingsPath } from "~/api/queries/settings-prefetch";
import { useMemberships, useOrgRights, useSession } from "~/auth/session";
import { t } from "~/i18n";
import { readLast } from "./last";
import { orgPath, placePath } from "./pages";
import { ACCOUNT_PAGES, INSTANCE_PAGES, ORG_SETTINGS_PAGES, type SettingsPage } from "./settings-pages";

export function SettingsSidebar() {
  const params = useParams({ strict: false });
  const session = useSession();
  const memberships = useMemberships();
  const last = readLast();
  const org = params.org ?? last?.org ?? memberships[0]?.org_id ?? "";
  const orgName = session.memberships.find((m) => m.org_id === org)?.org_name ?? t("common.organization", "Organization");
  const orgRights = useOrgRights(org);
  const [query, setQuery] = useState("");
  const pathname = useLocation({ select: (l) => l.pathname });
  const back = last ? placePath(last) : org ? orgPath(org) : "/";
  const match = (p: SettingsPage) => !query.trim() || p.label().toLowerCase().includes(query.trim().toLowerCase());
  const orgPages = org ? ORG_SETTINGS_PAGES.filter((p) => !p.right || orgRights.has(p.right)) : [];
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const prefetch = (e: SyntheticEvent) => {
    const href = (e.target as Element).closest("a")?.getAttribute("href");
    if (href) prefetchSettingsPath(queryClient, href);
  };
  const matches = [
    ...ACCOUNT_PAGES.filter(match).map((p) => `/settings/account/${p.path}`),
    ...orgPages.filter(match).map((p) => `/settings/orgs/${org}/${p.path}`),
    ...(session.user.is_instance_admin ? INSTANCE_PAGES.filter(match).map((p) => `/settings/instance/${p.path}`) : []),
  ];
  const anyMatch = matches.length > 0;
  const group = (label: string, base: string, pages: SettingsPage[]) => {
    const shown = pages.filter(match);
    if (!shown.length) return null;
    return (
      <Sidebar.Group label={label} collapsible={false}>
        {shown.map((p) => (
          <Sidebar.Item
            key={p.id}
            icon={p.icon}
            href={`${base}/${p.path}`}
            active={pathname === `${base}/${p.path}` || pathname.startsWith(`${base}/${p.path}/`)}
          >
            {p.label()}
          </Sidebar.Item>
        ))}
      </Sidebar.Group>
    );
  };
  return (
    <Sidebar aria-label={t("common.settings", "Settings")}>
      <Sidebar.Brand href={back} label={t("shell.backToConsole", "Back to the console")}>
        <Sidebar.CollapseToggle />
      </Sidebar.Brand>
      <Sidebar.Content onPointerOver={prefetch} onFocus={prefetch}>
        <div className="flex flex-col gap-8">
          <Sidebar.BackLink href={back}>{t("shell.backToApp", "Back to app")}</Sidebar.BackLink>
          <SearchInput
            aria-label={t("shell.searchSettings", "Search settings")}
            placeholder={t("shell.searchSettings", "Search settings")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && query.trim() && matches[0] && void navigate({ href: matches[0] })}
            size="sm"
          />
        </div>
        {group(t("shell.account", "Account"), "/settings/account", ACCOUNT_PAGES)}
        {org && group(orgName, `/settings/orgs/${org}`, orgPages)}
        {session.user.is_instance_admin && group(t("shell.instance", "Instance"), "/settings/instance", INSTANCE_PAGES)}
        {query.trim() && !anyMatch && (
          <Text variant="small" tone="secondary" className="mx-12">
            {t("shell.noSettingsMatch", "No settings match")}
          </Text>
        )}
      </Sidebar.Content>
    </Sidebar>
  );
}
