import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { AlertDialog, Avatar, Badge, Button, DialogScope, SettingsGroup, SettingsRow, SettingsSection, Text } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { api } from "~/api/client";
import type { MyMembership } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { useMemberships } from "~/auth/session";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { projectPath } from "~/shell/pages";
import { SettingsPage } from "~/shell/settings-page";
import { t } from "~/i18n";

export function MembershipsPage() {
  const memberships = useMemberships();
  const [leaving, setLeaving] = useState<MyMembership | null>(null);
  const [leaveOpen, setLeaveOpen] = useState(false);
  const leave = (m: MyMembership) => {
    setLeaving(m);
    setLeaveOpen(true);
  };
  return (
    <SettingsPage
      title={t("account.memberships", "Memberships")}
      context={t("account.title", "Account")}
      description={t("account.membershipsHint", "Your organizations, your rights in them, and their projects you belong to.")}
    >
      {memberships.length === 0 && <Text tone="secondary">{t("account.noMemberships", "You don't belong to an organization.")}</Text>}
      {memberships.map((m) => (
        <SettingsSection
          key={m.org_id}
          title={m.org_name}
          description={t("account.orgRole", "{role} of the organization", { role: m.role.name })}
          actions={m.org_state === "scheduled_for_deletion" ? undefined : <LeaveButton membership={m} onLeave={() => leave(m)} />}
        >
          <SettingsGroup aria-label={t("account.projectsIn", "Projects in {org}", { org: m.org_name })}>
            {m.org_state === "scheduled_for_deletion" && (
              <SettingsRow
                danger
                title={t("account.scheduledForDeletion", "Scheduled for deletion")}
                description={
                  m.org_deletion_due_at
                    ? t("account.readOnlyUntil", "Read-only until {date}; an owner can restore it.", { date: formatDate(m.org_deletion_due_at) })
                    : t("account.readOnlyUntilDeleted", "Read-only until it's deleted; an owner can restore it.")
                }
              />
            )}
            {m.projects.length === 0 ? (
              <SettingsRow title={t("account.noProjects", "No projects")} description={t("account.noProjectsHint", "You belong to none of its projects.")} />
            ) : (
              m.projects.map((p) => (
                <SettingsRow
                  key={p.id}
                  title={
                    <span className="flex items-center gap-8">
                      <Avatar name={p.name} {...(p.avatar_url ? { src: p.avatar_url } : {})} kind="org" size={20} decorative />
                      {p.name}
                      {p.is_default && <Badge>{t("account.defaultProject", "Default")}</Badge>}
                    </span>
                  }
                  value={p.role.name}
                  href={projectPath(m.org_id, p.id)}
                />
              ))
            )}
          </SettingsGroup>
        </SettingsSection>
      ))}
      {leaving && (
        <DialogScope open={leaveOpen}>
          <LeaveDialog membership={leaving} open={leaveOpen} onOpenChange={setLeaveOpen} />
        </DialogScope>
      )}
    </SettingsPage>
  );
}

function LeaveDialog({ membership, open, onOpenChange }: { membership: MyMembership; open: boolean; onOpenChange: (open: boolean) => void }) {
  const queryClient = useQueryClient();
  const leave = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/me/memberships/{org}", { params: { path: { org: membership.org_id } } });
    },
    onSuccess: () => {
      onOpenChange(false);
      toast(t("account.leftOrg", "You left {org}", { org: membership.org_name }));
      void queryClient.invalidateQueries({ queryKey: ["session"] });
      void queryClient.invalidateQueries({ queryKey: ["me"] });
    },
    onError: () => onOpenChange(false),
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("account.leaveConfirm", "Leave {org}?", { org: membership.org_name })}
      description={t("account.leaveConfirmHint", "You lose access to its projects and your keys in them are archived; an owner can invite you again.")}
      confirmLabel={t("account.leaveOrg", "Leave organization")}
      loading={leave.isPending}
      onConfirm={() => leave.mutateAsync()}
    />
  );
}

function LeaveButton({ membership: m, onLeave }: { membership: MyMembership; onLeave: () => void }) {
  return (
    <Button
      size="sm"
      variant="ghost"
      disabled={m.last_owner}
      disabledReason={m.last_owner ? t("account.soleOwner", "You're its only owner; make someone else an owner first.") : undefined}
      onClick={onLeave}
    >
      {t("account.leaveOrg", "Leave organization")}
    </Button>
  );
}
