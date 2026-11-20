import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Avatar, Badge, Button, Empty, FilterRow, List, Input, Select, icons } from "@logicplanes/kaeso-ui";
import { formatDate, formatNumber } from "@logicplanes/kaeso-ui/format";
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
import { t } from "~/i18n";
import { compareNames } from "~/format/collate";

export type ProjectsSort = "requests" | "cost";

export interface OrgProjectsSearch {
  create?: boolean;
  sort?: ProjectsSort;
}

export function OrgProjectsPage({ org, search }: { org: string; search: OrgProjectsSearch }) {
  const navigate = useNavigate();
  const rights = useOrgRights(org);
  const needs = useNeedsRight(org);
  const canCreate = rights.has("projects.create");
  const createReason = canCreate ? undefined : needs("projects.create");
  const [scope, setScope] = useState<Scope>("active");
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState(!!search.create && canCreate);
  const projects = useQuery(orgProjectsQuery(org, scope !== "active"));
  const last30 = resolvePeriod({});
  const seesUsage = rights.has("usage.org.view");
  const seesCost = seesUsage && rights.has("costs.org.view");
  const usage = useQuery({ ...orgUsageQuery(org, last30.from, last30.to, "project"), enabled: seesUsage });
  // Requests or cost in the same 30 days the subtitles show; without the right to see them, by name.
  const sort = seesUsage && (search.sort !== "cost" || seesCost) ? search.sort : undefined;
  usePageCreateAction({ label: t("orgHome.createProject", "Create project"), run: () => setCreating(true), disabledReason: createReason });

  const closeCreate = () => {
    setCreating(false);
    if (search.create) void navigate({ to: "/orgs/$org/projects", params: { org }, search: search.sort ? { sort: search.sort } : {}, replace: true });
  };
  const setSort = (v: string) =>
    void navigate({
      to: "/orgs/$org/projects",
      params: { org },
      search: v === "requests" || v === "cost" ? { sort: v } : {},
      replace: true,
    });
  const amount = (id: string): number => {
    const row = usage.data?.find((r) => r.key === id);
    return sort === "cost" ? Number(row?.converted_cost ?? 0) : (row?.requests ?? 0);
  };
  const all = [...(projects.data ?? [])].sort((a, b) => (sort ? amount(b.id) - amount(a.id) : 0) || compareNames(a.name, b.name));
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

  return (
    <>
      <OrgHeader
        org={org}
        page={t("common.projects", "Projects")}
        tabs={<ScopeTabs label={t("common.projects", "Projects")} value={scope} onChange={setScope} />}
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
      <FilterRow>
        <div className="min-w-0 flex-1 basis-160 sm:w-240 sm:flex-none">
          <Input
            type="search"
            size="sm"
            aria-label={t("orgHome.searchProjects", "Search projects")}
            placeholder={t("orgHome.projectName", "Project name")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onClear={() => setQuery("")}
          />
        </div>
        {seesUsage && (
          <Select
            aria-label={t("orgHome.sort", "Sort")}
            size="sm"
            icon={icons.sort}
            value={sort ?? "name"}
            onValueChange={setSort}
            options={[
              { value: "name", label: t("orgHome.sortName", "Name") },
              { value: "requests", label: t("orgHome.sortRequests", "Most requests") },
              ...(seesCost ? [{ value: "cost", label: t("orgHome.sortCost", "Highest cost") }] : []),
            ]}
          />
        )}
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
              actions={
                canCreate && (
                  <Button variant="primary" leadingIcon={icons.add} onClick={() => setCreating(true)}>
                    {t("orgHome.createProject", "Create project")}
                  </Button>
                )
              }
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
              meta: p.archived_at ? <Badge>{t("common.archived", "Archived")}</Badge> : undefined,
              date: p.archived_at
                ? t("orgHome.archivedOn", "Archived {date}", { date: formatDate(p.archived_at) })
                : t("orgHome.createdOn", "Created {date}", { date: formatDate(p.created_at) }),
            }))}
            onOpen={(item) => void navigate({ href: projectPath(org, item.id) })}
          />
        )}
      </AppFrame.Body>
      {creating && <CreateProjectDialog org={org} onClose={closeCreate} />}
    </>
  );
}
