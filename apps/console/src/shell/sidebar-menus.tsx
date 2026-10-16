import { useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate } from "@tanstack/react-router";
import { Menu, icons } from "@logicplanes/kaeso-ui";
import { api } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { ProjectView } from "~/api/generated/types";
import { useMemberships, useSession } from "~/auth/session";
import { t } from "~/i18n";
import { deletionCaption } from "./org-switcher";
import { orgPath, projectPath } from "./pages";
import { toast } from "./toast";
import { Picture } from "./project-switcher";

const PERIOD = ["from", "to", "by"] as const;

export function ProjectMenu({
  org,
  project,
  known,
  listed = [],
}: {
  org: string;
  project: string;
  known: ProjectView | undefined;
  listed: ProjectView[] | undefined;
}) {
  const navigate = useNavigate();
  const pathname = useLocation({ select: (l) => l.pathname });
  const section = pathname.slice(projectPath(org, project).length).split("/")[1] ?? "";
  const search = useLocation({ select: (l) => l.search as Record<string, unknown> });
  const kept = section
    ? ""
    : new URLSearchParams(
        PERIOD.flatMap((k) => {
          const v = search[k];
          return typeof v === "string" ? [[k, v]] : [];
        }),
      ).toString();
  const list = known && !listed.some((p) => p.id === project) ? [known, ...listed] : listed;
  return (
    <>
      <Menu.Group label={t("common.projects", "Projects")}>
        <Menu.RadioGroup
          value={project}
          onValueChange={(next) =>
            void navigate({ href: section ? `${projectPath(org, next)}/${section}` : `${projectPath(org, next)}${kept ? `?${kept}` : ""}` })
          }
        >
          {list.map((p) => (
            <Menu.RadioItem
              key={p.id}
              value={p.id}
              closeOnClick
              leading={<Picture project={p} />}
              {...(p.is_default ? { hint: t("shell.defaultProject", "Default") } : {})}
            >
              {p.name}
            </Menu.RadioItem>
          ))}
        </Menu.RadioGroup>
      </Menu.Group>
      <Menu.Separator />
      <Menu.LinkItem icon={icons.project} href={`${orgPath(org)}/projects`}>
        {t("shell.allProjects", "All projects")}
      </Menu.LinkItem>
    </>
  );
}

export function OrgMenu({ org, onCreate }: { org: string | undefined; onCreate: () => void }) {
  const session = useSession();
  const memberships = useMemberships();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const canCreate = session.can_create_orgs;

  const signOut = async () => {
    try {
      await api.POST("/platform/v1/auth/logout");
    } catch (e) {
      if (!(isApiError(e) && e.status === 401)) {
        toast.error(isApiError(e) ? e.message : t("common.unreachable", "Can't reach the gateway. Try again."));
        return;
      }
    }
    queryClient.clear();
    await navigate({ to: "/sign-in" });
  };

  return (
    <>
      {(memberships.length > 1 || canCreate) && (
        <>
          <Menu.Group label={t("shell.organizations", "Organizations")}>
            <Menu.RadioGroup value={org ?? ""} onValueChange={(next) => void navigate({ to: "/orgs/$org", params: { org: next } })}>
              {memberships.map((m) => (
                <Menu.RadioItem
                  key={m.org_id}
                  value={m.org_id}
                  closeOnClick
                  icon={icons.organization}
                  {...(m.org_state === "scheduled_for_deletion"
                    ? { description: deletionCaption(m.org_deletion_due_at), descriptionTone: "danger" as const }
                    : {})}
                >
                  {m.org_name}
                </Menu.RadioItem>
              ))}
            </Menu.RadioGroup>
            {canCreate && (
              <Menu.Item icon={icons.add} onClick={onCreate}>
                {t("shell.createOrg", "Create organization")}
              </Menu.Item>
            )}
          </Menu.Group>
          <Menu.Separator />
        </>
      )}
      {org && (
        <Menu.LinkItem icon={icons.settings} href={`/settings/orgs/${org}/general`}>
          {t("shell.orgSettings", "Organization settings")}
        </Menu.LinkItem>
      )}
      <Menu.LinkItem icon={icons.member} href="/settings/account/profile">
        {t("shell.account", "Account")}
      </Menu.LinkItem>
      {session.user.is_instance_admin && (
        <Menu.LinkItem icon={icons.instance} href="/settings/instance/users">
          {t("shell.instance", "Instance")}
        </Menu.LinkItem>
      )}
      <Menu.Separator />
      <Menu.Item icon={icons.signOut} onClick={() => void signOut()}>
        {t("shell.signOut", "Sign out")}
      </Menu.Item>
    </>
  );
}
