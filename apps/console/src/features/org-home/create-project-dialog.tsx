import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { Button, Dialog, Input } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import { projectListKey } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { projectPath } from "~/shell/pages";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

export function CreateProjectDialog({ org, onClose }: { org: string; onClose: () => void }) {
  const navigate = useNavigate();
  const [name, setName] = useState("");
  const create = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/projects", { params: { path: { org } }, body: { name: name.trim() } })),
    invalidates: [projectListKey(org), ["session"], ["me", "memberships"]],
    onSuccess: (p) => {
      toast.success(t("orgHome.projectCreated", "{name} created", { name: p.name }));
      onClose();
      void navigate({ href: projectPath(org, p.id) });
    },
  });
  const reason = name.trim() ? undefined : t("orgHome.projectNameNeeded", "Give the project a name");
  return (
    <Dialog
      busy={create.isPending}
      open
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={t("orgHome.createProject", "Create project")}
      description={t("orgHome.createProjectHint", "You become its admin and can add members, models and keys inside it.")}
      onSubmit={() => !reason && create.mutate()}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel", "Cancel")}</Button>
          <Button
            variant="primary"
            disabled={!!reason}
            {...(reason ? { disabledReason: reason } : {})}
            loading={create.isPending}
            onClick={() => create.mutate()}
          >
            {t("orgHome.createProject", "Create project")}
          </Button>
        </>
      }
    >
      <Input label={t("common.name", "Name")} value={name} onChange={(e) => setName(e.target.value)} maxLength={100} autoFocus />
    </Dialog>
  );
}
