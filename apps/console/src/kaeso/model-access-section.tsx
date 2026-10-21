import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button, Section, Text } from "@logicplanes/kaeso-ui";
import { ModelPicker } from "@logicplanes/kaeso-ui/playground";
import { toast } from "~/shell/toast";
import { api, data } from "~/api/client";
import type { ModelAccess, ModelAccessUpdate } from "~/api/generated/types";
import { catalogueQuery } from "~/api/queries/models";
import { modelAccessQuery, type Principal } from "~/api/queries/members";
import { joinList } from "~/format/labels";
import { modelName } from "~/format/models";
import { catalogueModelOption, keepChosen } from "./model-options";
import { useModelPickerProviders } from "./providers";
import { t } from "~/i18n";
import { useApiMutation } from "~/auth/mutation";

export type AccessSubject = Principal | { kind: "key"; id: string; owner: Principal };

export function ModelAccessSection({ org, project, subject, canEdit }: { org: string; project: string; subject: AccessSubject; canEdit: boolean }) {
  const self: Principal | { kind: "key"; id: string } = subject.kind === "key" ? { kind: "key", id: subject.id } : subject;
  const access = useQuery(modelAccessQuery(org, project, self));
  const owner = useQuery({
    ...modelAccessQuery(org, project, subject.kind === "key" ? subject.owner : { kind: "user", id: "" }),
    enabled: canEdit && subject.kind === "key",
  });
  const catalogue = useQuery(catalogueQuery(org, project));
  const [editing, setEditing] = useState<string[] | null>(null);
  const byId = (id: string) => catalogue.data?.find((m) => m.id === id);
  const name = (id: string) => {
    const m = byId(id);
    return m ? modelName(m) : id;
  };
  const choices = useMemo(
    () => (subject.kind === "key" ? (owner.data?.effective ?? []) : (catalogue.data ?? []).map((m) => m.id)),
    [subject.kind, owner.data, catalogue.data],
  );
  const options = useMemo(
    () =>
      choices.map((id) => {
        const m = catalogue.data?.find((x) => x.id === id);
        return m ? catalogueModelOption(m) : { value: id, provider: "", model: id };
      }),
    [choices, catalogue.data],
  );
  const providers = useModelPickerProviders(
    org,
    useMemo(() => [...new Set(options.map((o) => o.provider))], [options]),
  );
  const above =
    subject.kind === "key"
      ? t("kaeso.modelAccess.ownerModels", "Every model its owner may use")
      : t("kaeso.modelAccess.catalogueModels", "Every model in the catalogue");

  const save = useApiMutation({
    mutationFn: async (models: string[]): Promise<ModelAccess> => {
      const body: ModelAccessUpdate = models.length ? { models } : { all: true };
      const path = { org, project };
      if (subject.kind === "user")
        return data(
          await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/members/{user}/models", { params: { path: { ...path, user: subject.id } }, body }),
        );
      if (subject.kind === "service_account")
        return data(
          await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/service-accounts/{account}/models", {
            params: { path: { ...path, account: subject.id } },
            body,
          }),
        );
      return data(await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/keys/{key}/models", { params: { path: { ...path, key: subject.id } }, body }));
    },
    invalidates: [["orgs", org, "projects", project, "model-access"]],
    onSuccess: () => {
      setEditing(null);
      toast.success(t("kaeso.modelAccess.saved", "Models saved"));
    },
  });

  return (
    <Section
      title={t("common.models", "Models")}
      spacing="none"
      actions={
        canEdit && editing === null ? (
          <Button size="sm" variant="ghost" onClick={() => setEditing(access.data?.all ? [] : (access.data?.models ?? []))}>
            {t("common.edit", "Edit")}
          </Button>
        ) : undefined
      }
    >
      {editing !== null ? (
        <div className="flex flex-col gap-12">
          <ModelPicker
            aria-label={t("common.models", "Models")}
            multiple
            appearance="default"
            size="md"
            fullWidth
            placeholder={above}
            options={keepChosen(options, editing)}
            providers={providers}
            value={editing}
            onValueChange={setEditing}
          />
          {subject.kind !== "key" && (
            <Text variant="small" tone="secondary">
              {t("kaeso.modelAccess.narrowed", "Their keys' model lists are narrowed to match.")}
            </Text>
          )}
          <div className="flex gap-8">
            <Button size="sm" variant="primary" loading={save.isPending} onClick={() => save.mutate(editing)}>
              {t("common.save", "Save")}
            </Button>
            <Button size="sm" onClick={() => setEditing(null)}>
              {t("common.cancel", "Cancel")}
            </Button>
          </div>
        </div>
      ) : access.data?.all ? (
        <Text tone="secondary">
          {access.data.effective.length === 0
            ? t("kaeso.modelAccess.none", "No models to call yet.")
            : subject.kind === "key"
              ? t(
                  "kaeso.modelAccess.ownerCount",
                  "{count, plural, one {Every model its owner may use: # model.} other {Every model its owner may use: # models.}}",
                  {
                    count: access.data.effective.length,
                  },
                )
              : t(
                  "kaeso.modelAccess.catalogueCount",
                  "{count, plural, one {Every model in the catalogue: # model.} other {Every model in the catalogue: # models.}}",
                  {
                    count: access.data.effective.length,
                  },
                )}
        </Text>
      ) : (
        <Text>{joinList((access.data?.models ?? []).map(name)) || t("common.none", "None")}</Text>
      )}
    </Section>
  );
}
