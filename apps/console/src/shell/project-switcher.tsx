import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { Avatar, Sidebar } from "@logicplanes/kaeso-ui";
import type { ProjectView } from "~/api/generated/types";
import { orgProjectsQuery, projectQuery } from "~/api/queries/projects";
import { t } from "~/i18n";
import { preloadPictures } from "./pictures";
import { OpensWhenReady, useSidebarMenus } from "./sidebar-menus-load";

export function ProjectSwitcher({ org, project }: { org: string; project: string }) {
  const current = useQuery(projectQuery(org, project));
  const projects = useQuery(orgProjectsQuery(org));
  const listed = projects.data;
  const known = current.data ?? listed?.find((p) => p.id === project);
  const name = known?.name ?? "";
  const menus = useSidebarMenus();
  useEffect(() => preloadPictures(listed?.map((p) => p.avatar_url) ?? []), [listed]);
  return (
    <OpensWhenReady ready={!!menus && !projects.isPending}>
      <Sidebar.Place
        avatar={<Picture project={known} />}
        label={name}
        aria-label={name ? t("shell.projectSwitcher.label", "{name}: switch project", { name }) : t("shell.switchProject", "Switch project")}
      >
        {menus ? <menus.ProjectMenu org={org} project={project} known={known} listed={listed} /> : <></>}
      </Sidebar.Place>
    </OpensWhenReady>
  );
}

export function Picture({ project }: { project: Pick<ProjectView, "name" | "avatar_url"> | undefined }) {
  return <Avatar name={project?.name ?? ""} {...(project?.avatar_url ? { src: project.avatar_url } : {})} kind="org" size={20} decorative />;
}
