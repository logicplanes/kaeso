import { useState } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { Button, Combobox, IconButton, RolePicker, Select, StepDialog, Text, icons } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { useOrgGiver } from "~/auth/givers";
import { projectRightsQuery, useSession } from "~/auth/session";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { useCatalogue } from "~/kaeso/catalogue";
import { roleOptions, rolesOfScope, useRoles } from "~/kaeso/roles";
import { toast } from "~/shell/toast";

type ProjectRole = { project_id: string; role: string };

const looksLikeEmail = (s: string) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(s);

export function InviteDialog({ org, onClose }: { org: string; onClose: () => void }) {
  const projects = useQuery(orgProjectsQuery(org));
  const session = useSession();
  const cat = useCatalogue(org);
  const allRoles = useRoles(org);
  const giver = useOrgGiver(org);
  const [emails, setEmails] = useState<string[]>([]);
  const [role, setRole] = useState("member");
  const [roles, setRoles] = useState<ProjectRole[]>([]);
  const held = useQueries({ queries: roles.map((r) => projectRightsQuery(session, org, r.project_id)) });
  const orgRoles = rolesOfScope(allRoles, "org");
  const projectRoles = rolesOfScope(allRoles, "project");
  const extra = (projects.data ?? []).filter((p) => !p.is_default);
  const defaultProject = projects.data?.find((p) => p.is_default);

  const send = useApiMutation({
    mutationFn: async () => {
      const sent: string[] = [];
      for (const email of emails) {
        try {
          await api.POST("/platform/v1/orgs/{org}/invites", { params: { path: { org } }, body: { email, role, project_roles: roles } }).then(data);
          sent.push(email);
        } catch (error) {
          if (isApiError(error) && error.type !== "recent_sign_in_required")
            toast.error(t("org.invite.failed", "{email}: {message}", { email, message: error.message }));
          else throw error;
        }
      }
      return sent;
    },
    invalidates: [["orgs", org, "invites"]],
    onSuccess: (sent) => {
      const [first, ...others] = sent;
      if (!first) return;
      toast.success(
        others.length === 0
          ? t("org.invite.sent", "Invitation sent to {email}", { email: first })
          : t("org.invite.sentMany", "{count, plural, one {# invitation sent} other {# invitations sent}}", { count: sent.length }),
      );
      if (sent.length === emails.length) onClose();
      else setEmails(emails.filter((e) => !sent.includes(e)));
    },
  });

  const addEmail = (raw: string) => {
    const found = raw.split(/[\s,;]+/).filter(Boolean);
    const valid = found.filter(looksLikeEmail).map((e) => e.toLowerCase());
    const invalid = found.filter((e) => !looksLikeEmail(e));
    if (invalid.length)
      toast.warning(
        t("org.invite.notEmails", "{count, plural, one {{addresses} isn't an email address} other {{addresses} aren't email addresses}}", {
          count: invalid.length,
          addresses: invalid.join(", "),
        }),
      );
    if (valid.length) setEmails((list) => [...new Set([...list, ...valid])]);
  };
  const nextProject = extra.find((p) => !roles.some((r) => r.project_id === p.id));
  const reason = emails.length ? undefined : t("org.invite.addEmail", "Add at least one email address");

  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t("org.invite.title", "Invite members")}
      submitLabel={
        emails.length > 1
          ? t("org.invite.sendMany", "{count, plural, one {Send # invitation} other {Send # invitations}}", { count: emails.length })
          : t("org.invite.send", "Send invitation")
      }
      submitting={send.isPending}
      onSubmit={() => !reason && send.mutate()}
      steps={[
        {
          id: "who",
          title: t("org.invite.who", "Who are you inviting?"),
          description: t("org.invite.whoHint", "Type an address and press Enter, or paste several; each link works for 21 days."),
          blockedReason: reason,
          content: (
            <Combobox
              multiple
              aria-label={t("org.invite.emails", "Email addresses")}
              placeholder={t("org.invite.emailExample", "name@example.com")}
              options={emails.map((e) => ({ value: e, label: e }))}
              value={emails}
              onValueChange={setEmails}
              onCreate={addEmail}
              emptyText={t("org.invite.typeAddress", "Type a full address")}
            />
          ),
        },
        {
          id: "role",
          title: t("org.invite.role", "What can they do in the organization?"),
          description: t("org.invite.roleHint", "Their role; you can change it, or customize their rights, once they join."),
          content: (
            <RolePicker
              aria-label={t("org.invite.orgRole", "Role in the organization")}
              roles={roleOptions(cat, orgRoles, giver)}
              value={role}
              onValueChange={setRole}
            />
          ),
        },
        {
          id: "projects",
          title: t("org.invite.projects", "Which projects?"),
          description: defaultProject
            ? t("org.invite.projectsHint", "They join {project}, the default project; add others now or later.", { project: defaultProject.name })
            : t("org.invite.projectsHintUnnamed", "They join the default project; add others now or later."),
          content: (
            <div className="flex flex-col gap-12">
              {roles.map((pr, i) => (
                <div key={pr.project_id} className="flex items-center gap-8">
                  <div className="min-w-0 flex-1 basis-140">
                    <Select
                      aria-label={common.project()}
                      fullWidth
                      value={pr.project_id}
                      onValueChange={(project_id) => setRoles(roles.map((r, j) => (j === i ? { ...r, project_id } : r)))}
                      options={extra
                        .filter((p) => p.id === pr.project_id || !roles.some((r) => r.project_id === p.id))
                        .map((p) => ({ value: p.id, label: p.name }))}
                    />
                  </div>
                  <div className="min-w-0 flex-1 basis-140">
                    <RolePicker
                      appearance="select"
                      aria-label={t("org.invite.projectRole", "Role in the project")}
                      fullWidth
                      value={pr.role}
                      onValueChange={(next) => setRoles(roles.map((r, j) => (j === i ? { ...r, role: next } : r)))}
                      roles={roleOptions(cat, projectRoles, held[i]?.data ? { held: held[i].data, isOwner: giver.isOwner } : null)}
                    />
                  </div>
                  <IconButton
                    icon={icons.close}
                    aria-label={t("org.invite.removeProject", "Remove this project")}
                    size="sm"
                    onClick={() => setRoles(roles.filter((_, j) => j !== i))}
                  />
                </div>
              ))}
              {nextProject ? (
                <div className="flex">
                  <Button size="sm" leadingIcon={icons.add} onClick={() => setRoles([...roles, { project_id: nextProject.id, role: "developer" }])}>
                    {t("org.invite.addProject", "Add a project")}
                  </Button>
                </div>
              ) : (
                roles.length === 0 && (
                  <Text variant="small" tone="secondary">
                    {t("org.invite.noOtherProjects", "The organization has no other projects.")}
                  </Text>
                )
              )}
            </div>
          ),
        },
      ]}
    />
  );
}
