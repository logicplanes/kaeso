import { AlertDialog } from "@logicplanes/kaeso-ui";
import { api } from "~/api/client";
import type { AliasView } from "~/api/generated/types";
import { projectCatalogue } from "~/api/queries/invalidation";
import { aliasQuery } from "~/api/queries/models";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

export function DeleteAliasDialog({
  org,
  project,
  alias,
  open,
  onOpenChange,
  onDeleted,
}: {
  org: string;
  project: string;
  alias: Pick<AliasView, "id" | "name">;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDeleted?: (() => void) | undefined;
}) {
  const startsWith = (key: readonly unknown[], prefix: readonly unknown[]) => prefix.every((part, i) => key[i] === part);
  const gone = aliasQuery(org, project, alias.id).queryKey;
  const remove = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/orgs/{org}/projects/{project}/aliases/{alias}", { params: { path: { org, project, alias: alias.id } } });
    },
    invalidates: [({ queryKey }) => projectCatalogue(org, project).some((k) => startsWith(queryKey, k)) && !startsWith(queryKey, gone)],
    onSuccess: () => {
      toast(t("models.alias.deleted", "Alias {name} deleted", { name: alias.name }));
      onOpenChange(false);
      onDeleted?.();
    },
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("models.alias.deleteTitle", "Delete the alias {name}?", { name: alias.name })}
      description={t("models.alias.deleteHint", "Requests that send this name are refused, unless a model has the same name.")}
      confirmLabel={t("models.alias.delete", "Delete alias")}
      loading={remove.isPending}
      onConfirm={() => remove.mutateAsync()}
    />
  );
}
