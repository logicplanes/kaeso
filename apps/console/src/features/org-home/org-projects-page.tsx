import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Avatar, Badge, Button, Empty, FilterRow, IconButton, List, Menu, SearchInput, icons } from "@logicplanes/kaeso-ui";
import { formatDate, formatNumber } from "@logicplanes/kaeso-ui/format";
import type { ProjectView } from "~/api/generated/types";
import { orgProjectsQuery } from "~/api/queries/projects";
import { orgUsageQuery } from "~/api/queries/usage";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { LoadFailed } from "~/features/org/load-failed";
import { usePageCreateAction } from "~/shell/create-action";
import { ScopeTabs, type Scope } from "~/kaeso/scope-tabs";
import { resolvePeriod } from "~/format/presets";
import { OrgHeader } from "~/shell/org-header";
import { projectPath } from "~/shell/pages";
import { CreateProjectDialog } from "./create-project-dialog";
import { MakeDefaultDialog } from "./make-default-dialog";
import { t } from "~/i18n";
import { compareNames } from "~/format/collate";

export interface OrgProjectsSearch {
  create?: boolean;
}

export function OrgProjectsPage({ org, search }: { org: string; search: OrgProjectsSearch }) {
  const navigate = useNavigate();
  const rights = useOrgRights(org);
  const needs = useNeedsRight(org);
  const canCreate = rights.has("projects.create");
  const createReason = canCreate ? undefined : needs("projects.create");
  const canChooseDefault = rights.has("org.update");
  const [scope, setScope] = useState<Scope>("active");
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState(!!search.create && canCreate);
  const [promoting, setPromoting] = useState<ProjectView | null>(null);
  const projects = useQuery(orgProjectsQuery(org, scope !== "active"));
  const last30 = resolvePeriod({});
  const usage = useQuery({ ...orgUsageQuery(org, last30.from, last30.to, "project"), enabled: rights.has("usage.org.view") });
  usePageCreateAction({ label: t("orgHome.createProject", "Create project"), run: () => setCreating(true), disabledReason: createReason });

  const closeCreate = () => {
    setCreating(false);
    if (search.create) void navigate({ to: "/orgs/$org/projects", params: { org }, search: {}, replace: true });
  };
  const all = [...(projects.data ?? [])].sort((a, b) => Number(b.is_default) - Number(a.is_default) || compareNames(a.name, b.name));
  const current = all.find((p) => p.is_default);
  const words = query.trim().toLocaleLowerCase();
  const list = all.filter((p) => (scope !== "archived" || p.archived_at) && (!words || p.name.toLocaleLowerCase().includes(words)));
  const requests = (id: string) => {
    if (!usage.data) return undefined;
    const n = usage.data.find((r) => r.key === id)?.requests ?? 0;
    return n
      ? t("orgHome.requestsIn30Days", "{count, plural, one {# request in 30 days} other {{formatted} requests in 30 days}}", {
          count: n,
          formatted: formatNumber(n),
        })
      : t("orgHome.noRequestsIn30Days", "No requests in 30 days");
  };

  const menu = (p: ProjectView) => {
    if (!canChooseDefault || p.is_default || p.archived_at) return undefined;
    return (
      <Menu>
        <Menu.Trigger>
          <IconButton icon={icons.more} aria-label={t("orgHome.actionsFor", "Actions for {name}", { name: p.name })} size="xs" />
        </Menu.Trigger>
        <Menu.Content align="end">
          <Menu.Item icon={icons.home} onClick={() => setPromoting(p)}>
            {t("orgHome.makeDefault", "Make default")}
          </Menu.Item>
        </Menu.Content>
      </Menu>
    );
  };

  return (
    <>
      <OrgHeader
        org={org}
        page={t("common.projects", "Projects")}
        actions={
          <Button
            size="sm"
            variant="primary"
            leadingIcon={icons.add}
            disabled={!canCreate}
            {...(createReason ? { disabledReason: createReason } : {})}
            onClick={() => setCreating(true)}
          >
            {t("orgHome.createProject", "Create project")}
          </Button>
        }
      />
      <FilterRow tabs={<ScopeTabs label={t("common.projects", "Projects")} value={scope} onChange={setScope} />}>
        <div className="w-240">
          <SearchInput
            aria-label={t("orgHome.searchProjects", "Search projects")}
            placeholder={t("orgHome.projectName", "Project name")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onClear={() => setQuery("")}
          />
        </div>
      </FilterRow>
      <AppFrame.Body padding="list">
        {projects.isError && all.length === 0 ? (
          <LoadFailed title={t("orgHome.projectsFailed", "The projects couldn't load")} />
        ) : !projects.isPending && list.length === 0 ? (
          words ? (
            <Empty
              size="md"
              title={t("orgHome.noProjectsMatch", "No projects match")}
              description={t("orgHome.searchAnotherProject", "Search by another project name.")}
            />
          ) : scope === "archived" ? (
            <Empty size="md" title={t("orgHome.noArchivedProjects", "No archived projects")} />
          ) : (
            <Empty
              title={t("orgHome.noProjects", "No projects you can see")}
              description={t("orgHome.noProjectsHint", "Projects you belong to are listed here.")}
            />
          )
        ) : (
          <List
            aria-label={t("common.projects", "Projects")}
            loading={projects.isPending}
            items={list.map((p) => ({
              id: p.id,
              textValue: p.name,
              title: p.name,
              subtitle: requests(p.id),
              leading: <Avatar name={p.name} {...(p.avatar_url ? { src: p.avatar_url } : {})} kind="org" size={20} decorative />,
              meta: p.is_default ? (
                <Badge>{t("orgHome.default", "Default")}</Badge>
              ) : p.archived_at ? (
                <Badge>{t("common.archived", "Archived")}</Badge>
              ) : undefined,
              date: p.archived_at
                ? t("orgHome.archivedOn", "Archived {date}", { date: formatDate(p.archived_at) })
                : t("orgHome.createdOn", "Created {date}", { date: formatDate(p.created_at) }),
              actions: menu(p),
            }))}
            onOpen={(item) => void navigate({ href: projectPath(org, item.id) })}
          />
        )}
      </AppFrame.Body>
      {creating && <CreateProjectDialog org={org} onClose={closeCreate} />}
      {promoting && <MakeDefaultDialog org={org} project={promoting} current={current} onClose={() => setPromoting(null)} />}
    </>
  );
}
