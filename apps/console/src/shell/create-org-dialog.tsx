import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Button, Dialog, Input } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { api, data } from "~/api/client";
import { fieldError } from "~/api/field-error";
import { useApiMutation } from "~/auth/mutation";
import { membershipsQuery, sessionQuery } from "~/auth/session";
import { t } from "~/i18n";

export default function CreateOrgDialog({ onClose }: { onClose: () => void }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const create = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/me/orgs", { body: { name: name.trim() } })),
    invalidates: [sessionQuery.queryKey, membershipsQuery.queryKey],
    onSuccess: async (created) => {
      toast.success(t("shell.createOrg.done", "Organization created"));
      onClose();
      await navigate({ to: `/orgs/${created.id}` });
    },
    onError: (e) => {
      if (e.type === "permission_denied") {
        void queryClient.invalidateQueries({ queryKey: sessionQuery.queryKey });
        onClose();
      }
    },
  });
  return (
    <Dialog
      busy={create.isPending}
      open
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={t("shell.createOrg", "Create organization")}
      description={t("shell.createOrg.description", "You become its owner; it starts with a default project.")}
      onSubmit={() => name.trim() && create.mutate()}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel", "Cancel")}</Button>
          <Button variant="primary" loading={create.isPending} disabled={!name.trim()} onClick={() => create.mutate()}>
            {t("shell.createOrg", "Create organization")}
          </Button>
        </>
      }
    >
      <Input
        label={t("common.name", "Name")}
        value={name}
        onChange={(e) => {
          setName(e.target.value);
          if (create.isError) create.reset();
        }}
        error={fieldError(create.error, "name")}
        autoFocus
        maxLength={100}
      />
    </Dialog>
  );
}
