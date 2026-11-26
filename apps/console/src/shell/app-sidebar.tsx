import { useQuery } from "@tanstack/react-query";
import { useLocation, useParams } from "@tanstack/react-router";
import { Sidebar, icons } from "@logicplanes/kaeso-ui";
import { isApiError } from "~/api/errors";
import { orgProjectsQuery, projectQuery } from "~/api/queries/projects";
import { useMemberships, useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { readLast } from "./last";
import { openPalette } from "./palette-store";
import { AccountSwitcher, OrgSwitcher } from "./org-switcher";
import { Picture, ProjectSwitcher } from "./project-switcher";
import { ORG_AREA_PAGES, orgPath, projectPath } from "./pages";
import { useProjectPages } from "./project-pages";

export function AppSidebar() {
  const params = useParams({ strict: false });
  const memberships = useMemberships();
  const last = readLast();
  const org = params.org ?? last?.org ?? memberships[0]?.org_id;
  const project = params.project;
  const opened = useQuery({ ...projectQuery(org ?? "", project ?? ""), enabled: !!org && !!project });
  const gone = isApiError(opened.error) && (opened.error.type === "not_found" || opened.error.type === "permission_denied");
  return (
    <Sidebar aria-label={t("shell.sidebar", "Console")}>
      <Sidebar.Brand lead={org && project && !gone ? <ProjectSwitcher org={org} project={project} /> : org ? <OrgSwitcher org={org} /> : undefined}>
        <Sidebar.CollapseToggle />
      </Sidebar.Brand>
      <Sidebar.Content>
        <Sidebar.Group>
          <Sidebar.Item icon={icons.search} shortcut={["mod", "k"]} onClick={openPalette}>
            {t("common.search", "Search")}
          </Sidebar.Item>
        </Sidebar.Group>
        {org && project && !gone ? <ProjectArea org={org} project={project} /> : org && <OrgArea org={org} />}
      </Sidebar.Content>
      <Sidebar.Footer>
        <AccountSwitcher />
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
            <Sidebar.Item
              key={p.id}
              icon={p.icon}
              href={href}
              active={pathname === href || pathname === `${href}/` || (!!p.subpages && pathname.startsWith(`${href}/`))}
            >
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
  const pages = useProjectPages(org, project);
  const pathname = useLocation({ select: (l) => l.pathname });
  const base = projectPath(org, project);
  return (
    <div className="flex flex-col gap-4">
      <Sidebar.BackLink href={`${orgPath(org)}/projects`}>{t("shell.allProjects", "All projects")}</Sidebar.BackLink>
      <Sidebar.Group>
        {pages.map((p) => {
          const href = p.path ? `${base}/${p.path}` : base;
          const active = p.path ? pathname === href || pathname.startsWith(`${href}/`) : pathname === base;
          return (
            <Sidebar.Item key={p.id} icon={p.icon} href={href} active={active} shortcut={[["g"], [p.key]]}>
              {p.label()}
            </Sidebar.Item>
          );
        })}
      </Sidebar.Group>
    </div>
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
