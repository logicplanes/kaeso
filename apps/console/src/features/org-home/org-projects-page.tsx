import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Avatar, Badge, Button, Empty, IconButton, List, Menu, icons } from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import type { ProjectView } from "~/api/generated/types";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useOrgRights } from "~/auth/session";
import { usePageCreateAction } from "~/shell/create-action";
import { ScopeTabs } from "~/kaeso/scope-tabs";
import { OrgHeader } from "~/shell/org-header";
import { projectPath } from "~/shell/pages";
import { CreateProjectDialog } from "./create-project-dialog";
import { MakeDefaultDialog } from "./make-default-dialog";
import { t } from "~/i18n";

export interface OrgProjectsSearch {
  create?: boolean;
}

export function OrgProjectsPage({ org, search }: { org: string; search: OrgProjectsSearch }) {
  const navigate = useNavigate();
  const rights = useOrgRights(org);
  const canCreate = rights.has("projects.create");
  const canChooseDefault = rights.has("org.update");
  const [archived, setArchived] = useState(false);
  const [creating, setCreating] = useState(!!search.create && canCreate);
  const [promoting, setPromoting] = useState<ProjectView | null>(null);
  const projects = useQuery(orgProjectsQuery(org, archived));
  usePageCreateAction(canCreate ? { label: t("orgHome.createProject", "Create project"), run: () => setCreating(true) } : null);

  const closeCreate = () => {
    setCreating(false);
    if (search.create) void navigate({ to: "/orgs/$org/projects", params: { org }, search: {}, replace: true });
  };
  const list = [...(projects.data ?? [])].sort((a, b) => Number(b.is_default) - Number(a.is_default) || a.name.localeCompare(b.name));
  const current = list.find((p) => p.is_default);

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
        tabs={<ScopeTabs label={t("common.projects", "Projects")} all={archived} onChange={setArchived} />}
        actions={
          canCreate && (
            <Button size="sm" variant="primary" leadingIcon={icons.add} onClick={() => setCreating(true)}>
              {t("orgHome.createProject", "Create project")}
            </Button>
          )
        }
      />
      <AppFrame.Body padding="list">
        {!projects.isPending && list.length === 0 ? (
          <Empty
            title={t("orgHome.noProjects", "No projects you can see")}
            description={t("orgHome.noProjectsHint", "Projects you belong to are listed here.")}
          />
        ) : (
          <List
            aria-label={t("common.projects", "Projects")}
            loading={projects.isPending}
            items={list.map((p) => ({
              id: p.id,
              textValue: p.name,
              title: p.name,
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
