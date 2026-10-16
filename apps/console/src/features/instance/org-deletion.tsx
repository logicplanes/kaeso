import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { Input, KeyValue, StepDialog } from "@logicplanes/kaeso-ui";
import { formatDate, formatNumber } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { InstanceOrgView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

const TOUCHED = [["instance", "orgs"], ["session"], ["me", "memberships"]];

export function useRestoreOrg() {
  return useApiMutation({
    mutationFn: async (org: string) => data(await api.POST("/platform/v1/instance/orgs/{org}/restore", { params: { path: { org } } })),
    invalidates: TOUCHED,
    onSuccess: (o) => toast.success(t("instance.orgRestored", "{name} restored", { name: o.name })),
  });
}

export function DeleteNowDialog({ org, onClose, inTable = false }: { org: InstanceOrgView; onClose: () => void; inTable?: boolean }) {
  const navigate = useNavigate();
  const [typed, setTyped] = useState("");
  const remove = useApiMutation({
    mutationFn: async () => (await api.DELETE("/platform/v1/instance/orgs/{org}", { params: { path: { org: org.id } } })).response.status,
    invalidates: TOUCHED,
    onSuccess: (status) => {
      toast(
        status === 202
          ? t("instance.orgDeletedSoon", "{name} is deleted within minutes", { name: org.name })
          : t("instance.orgDeleted", "{name} deleted", { name: org.name }),
      );
      onClose();
      if (!inTable) void navigate({ to: "/settings/instance/orgs" });
    },
    onError: () => onClose(),
  });
  const matches = typed.trim() === org.name;
  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t("instance.deleteOrgNow", "Delete {name} now", { name: org.name })}
      tone="danger"
      submitLabel={t("instance.deleteNow", "Delete now")}
      submitting={remove.isPending}
      onSubmit={() => matches && remove.mutate()}
      steps={[
        {
          id: "what",
          title: t("instance.whatIsDeleted", "What is deleted"),
          description: t("instance.whatIsDeletedHint", "Projects, keys, provider keys, settings, usage and logs, for good."),
          content: (
            <KeyValue
              items={[
                { label: t("common.organization", "Organization"), value: org.name },
                { label: t("common.members", "Members"), value: formatNumber(org.members) },
                { label: t("common.projects", "Projects"), value: formatNumber(org.projects) },
                ...(org.deletion_due_at ? [{ label: t("instance.scheduledFor", "Scheduled for"), value: formatDate(org.deletion_due_at) }] : []),
              ]}
            />
          ),
        },
        {
          id: "confirm",
          title: t("instance.typeNameToConfirm", "Type its name to confirm"),
          description: t("instance.deleteNowConfirmHint", "This can't be undone; members lose it at once."),
          blockedReason: matches ? undefined : t("instance.typeNameBlocked", "Type {name} to confirm", { name: org.name }),
          content: (
            <Input
              aria-label={t("instance.orgName", "Organization name")}
              placeholder={org.name}
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              autoComplete="off"
            />
          ),
        },
      ]}
    />
  );
}
