import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertDialog, Badge, Banner, Button, Dialog, Empty, IconButton, Input, List, Menu, Select, Text, Tooltip, icons } from "@logicplanes/kaeso-ui";
import { modelName } from "~/format/models";
import { toast } from "~/shell/toast";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { AliasView } from "~/api/generated/types";
import { aliasesQuery, catalogueQuery } from "~/api/queries/models";
import { useApiMutation } from "~/auth/mutation";
import { t } from "~/i18n";
import { common } from "~/i18n/common";

const shadowText = (name: string) => t("models.alias.shadows", "Apps that send `{name}` get this alias, not the model with the same name.", { name });

export function AliasesTab({
  org,
  project,
  canManage,
  creating,
  onCreatingChange,
}: {
  org: string;
  project: string;
  canManage: boolean;
  creating: boolean;
  onCreatingChange: (open: boolean) => void;
}) {
  const aliases = useQuery(aliasesQuery(org, project));
  const catalogue = useQuery(catalogueQuery(org, project));
  const targetName = (a: AliasView) => {
    const m = catalogue.data?.find((c) => c.id === a.catalogue_model_id);
    return m && modelName(m) !== m.model_id ? modelName(m) : undefined;
  };
  const [editing, setEditing] = useState<AliasView>();
  const [deleting, setDeleting] = useState<AliasView | null>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const askDelete = (a: AliasView) => {
    setDeleting(a);
    setDeleteOpen(true);
  };
  const invalidates = [
    ["orgs", org, "projects", project, "aliases"],
    ["orgs", org, "projects", project, "models"],
  ];
  const remove = useApiMutation({
    mutationFn: async (a: AliasView) => {
      await api.DELETE("/platform/v1/orgs/{org}/projects/{project}/aliases/{alias}", { params: { path: { org, project, alias: a.id } } });
    },
    invalidates,
    onSuccess: (_r, a) => {
      toast(t("models.alias.deleted", "Alias {name} deleted", { name: a.name }));
      setDeleteOpen(false);
    },
  });

  const list = aliases.data ?? [];
  return (
    <>
      {aliases.isPending ? (
        <List aria-label={t("models.aliases", "Aliases")} items={[]} loading />
      ) : list.length === 0 ? (
        <Empty
          size="lg"
          title={t("models.alias.empty", "No aliases yet")}
          description={t("models.alias.emptyHint", "A name like fast that apps send as model; retarget it and every app follows.")}
          actions={
            canManage ? (
              <Button variant="primary" onClick={() => onCreatingChange(true)}>
                {t("models.createAlias", "Create alias")}
              </Button>
            ) : undefined
          }
        />
      ) : (
        <List
          aria-label={t("models.aliases", "Aliases")}
          onOpen={(item) => canManage && setEditing(list.find((a) => a.id === item.id))}
          items={list.map((a) => ({
            id: a.id,
            textValue: a.name,
            title: <Text mono>{a.name}</Text>,
            subtitle: (
              <span className="flex min-w-0 items-baseline gap-6">
                <Text variant="small" tone="secondary" truncate>
                  → {targetName(a) ?? ""}
                </Text>
                <Text variant="small" tone="secondary" mono truncate className="flex-1 basis-0">
                  {a.target}
                </Text>
              </span>
            ),
            meta: a.shadows_model ? (
              <Tooltip content={shadowText(a.name)}>
                <Badge tone="warning" tabIndex={0}>
                  {t("models.alias.shadowsModel", "Shadows a model")}
                </Badge>
              </Tooltip>
            ) : undefined,
            date: formatRelative(a.updated_at),
            actions: canManage ? (
              <Menu>
                <Menu.Trigger>
                  <IconButton icon={icons.more} aria-label={t("models.alias.actionsFor", "Actions for {name}", { name: a.name })} size="xs" />
                </Menu.Trigger>
                <Menu.Content align="end">
                  <Menu.Item icon={icons.edit} onClick={() => setEditing(a)}>
                    {common.edit()}
                  </Menu.Item>
                  <Menu.Item icon={icons.delete} variant="danger" onClick={() => askDelete(a)}>
                    {common.delete()}
                  </Menu.Item>
                </Menu.Content>
              </Menu>
            ) : undefined,
          }))}
        />
      )}
      {creating && <AliasDialog org={org} project={project} onClose={() => onCreatingChange(false)} />}
      {editing && <AliasDialog org={org} project={project} alias={editing} onClose={() => setEditing(undefined)} />}
      {deleting && (
        <AlertDialog
          open={deleteOpen}
          onOpenChange={setDeleteOpen}
          tone="danger"
          title={t("models.alias.deleteTitle", "Delete the alias {name}?", { name: deleting.name })}
          description={t("models.alias.deleteHint", "Requests that send this name are refused, unless a model has the same name.")}
          confirmLabel={t("models.alias.delete", "Delete alias")}
          loading={remove.isPending}
          onConfirm={() => remove.mutate(deleting)}
        />
      )}
    </>
  );
}

function AliasDialog({ org, project, alias, onClose }: { org: string; project: string; alias?: AliasView; onClose: () => void }) {
  const catalogue = useQuery(catalogueQuery(org, project));
  const [name, setName] = useState(alias?.name ?? "");
  const [target, setTarget] = useState<string | null>(alias?.catalogue_model_id ?? null);
  const models = catalogue.data ?? [];
  const shadows = models.some((m) => m.model_id === name.trim());

  const save = useApiMutation({
    mutationFn: async (to: string) => {
      if (alias) {
        return data(
          await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/aliases/{alias}", {
            params: { path: { org, project, alias: alias.id } },
            body: {
              ...(name.trim() !== alias.name ? { name: name.trim() } : {}),
              ...(to !== alias.catalogue_model_id ? { catalogue_model_id: to } : {}),
            },
          }),
        );
      }
      return data(
        await api.POST("/platform/v1/orgs/{org}/projects/{project}/aliases", {
          params: { path: { org, project } },
          body: { name: name.trim(), catalogue_model_id: to },
        }),
      );
    },
    invalidates: [
      ["orgs", org, "projects", project, "aliases"],
      ["orgs", org, "projects", project, "models"],
    ],
    onSuccess: (a) => {
      toast(alias ? t("models.alias.saved", "Alias {name} saved", { name: a.name }) : t("models.alias.created", "Alias {name} created", { name: a.name }));
      onClose();
    },
  });
  const changed = !alias || name.trim() !== alias.name || target !== alias.catalogue_model_id;
  const ready = name.trim().length > 0 && changed ? target : null;
  const submit = () => ready && save.mutate(ready);

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={alias ? t("models.alias.editTitle", "Edit {name}", { name: alias.name }) : t("models.createAlias", "Create alias")}
      description={t("models.alias.dialogHint", "Apps send the alias as model and requests use its target, which can change at any time.")}
      onSubmit={submit}
      footer={
        <>
          <Button onClick={onClose}>{common.cancel()}</Button>
          <Button variant="primary" disabled={!ready} loading={save.isPending} onClick={submit}>
            {alias ? t("models.alias.save", "Save alias") : t("models.createAlias", "Create alias")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-20">
        <Input
          label={common.name()}
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoFocus
          maxLength={200}
          placeholder={t("models.alias.nameExample", "fast")}
        />
        <Select
          label={t("models.alias.target", "Target")}
          fullWidth
          placeholder={t("models.chooseModel", "Choose a model")}
          value={target}
          onValueChange={setTarget}
          options={models.map((m) => ({ value: m.id, label: modelName(m), description: `${m.provider}/${m.model_id}` }))}
        />
        {shadows && name.trim() && <Banner tone="warning">{shadowText(name.trim())}</Banner>}
      </div>
    </Dialog>
  );
}
