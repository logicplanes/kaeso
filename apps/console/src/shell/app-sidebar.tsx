import { useQuery } from "@tanstack/react-query";
import { useLocation, useParams } from "@tanstack/react-router";
import { IconButton, Sidebar, icons } from "@logicplanes/kaeso-ui";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useMembership, useMemberships, useOrgRights, useRequestLogs } from "~/auth/session";
import { t } from "~/i18n";
import { readLast } from "./last";
import { openPalette } from "./palette-store";
import { OrgSwitcher } from "./org-switcher";
import { Picture, ProjectSwitcher } from "./project-switcher";
import { ORG_AREA_PAGES, orgPath, projectPages, projectPath } from "./pages";
import { useProjectRights } from "~/auth/rights";

export function AppSidebar() {
  const params = useParams({ strict: false });
  const memberships = useMemberships();
  const last = readLast();
  const org = params.org ?? last?.org ?? memberships[0]?.org_id;
  const project = params.project;
  return (
    <Sidebar aria-label={t("shell.sidebar", "Console")}>
      <Sidebar.Brand href={org ? orgPath(org) : "/"} label={t("shell.home", "Kaeso home")}>
        <IconButton icon={icons.search} aria-label={t("common.search", "Search")} shortcut={["mod", "k"]} onClick={openPalette} />
        <Sidebar.CollapseToggle />
      </Sidebar.Brand>
      <Sidebar.Content>{org && project ? <ProjectArea org={org} project={project} /> : org && <OrgArea org={org} />}</Sidebar.Content>
      <Sidebar.Footer>
        <OrgSwitcher org={org} />
      </Sidebar.Footer>
    </Sidebar>
  );
}

function OrgArea({ org }: { org: string }) {
  const rights = useOrgRights(org);
  const pathname = useLocation({ select: (l) => l.pathname });
  const base = orgPath(org);
  return (
    <>
      <Sidebar.Group>
        {ORG_AREA_PAGES.filter((p) => rights.has(p.right)).map((p) => {
          const href = p.path ? `${base}/${p.path}` : base;
          return (
            <Sidebar.Item key={p.id} icon={p.icon} href={href} active={pathname === href || pathname === `${href}/`}>
              {p.label()}
            </Sidebar.Item>
          );
        })}
      </Sidebar.Group>
      <Projects org={org} />
    </>
  );
}

function ProjectArea({ org, project }: { org: string; project: string }) {
  const rights = useProjectRights(org, project);
  const requestLogs = useRequestLogs();
  const pathname = useLocation({ select: (l) => l.pathname });
  const membership = useMembership(org);
  const base = projectPath(org, project);
  return (
    <>
      <Sidebar.BackLink href={orgPath(org)}>{membership?.org_name ?? t("common.organization", "Organization")}</Sidebar.BackLink>
      <div className="flex flex-col gap-4">
        <ProjectSwitcher org={org} project={project} />
        <Sidebar.Group>
          {projectPages(requestLogs)
            .filter((p) => rights.has(p.right))
            .map((p) => {
              const href = p.path ? `${base}/${p.path}` : base;
              const active = p.path ? pathname === href || pathname.startsWith(`${href}/`) : pathname === base;
              return (
                <Sidebar.Item key={p.id} icon={p.icon} href={href} active={active} {...(p.shortcut ? { shortcut: p.shortcut } : {})}>
                  {p.label()}
                </Sidebar.Item>
              );
            })}
        </Sidebar.Group>
      </div>
    </>
  );
}

function Projects({ org }: { org: string }) {
  const projects = useQuery(orgProjectsQuery(org));
  const list = projects.data ?? [];
  return (
    <Sidebar.Group label={t("common.projects", "Projects")} id="projects">
      {projects.isPending && <Sidebar.Loading rows={2} />}
      {list.map((p) => (
        <Sidebar.Item key={p.id} leading={<Picture project={p} />} href={projectPath(org, p.id)}>
          {p.name}
        </Sidebar.Item>
      ))}
    </Sidebar.Group>
  );
}
