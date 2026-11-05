import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { LoadFailed } from "~/features/org/load-failed";
import { Badge, Button, Code, DialogScope, Empty, List, Menu, Text, Tooltip, icons } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import type { AliasView } from "~/api/generated/types";
import { aliasesQuery, catalogueQuery } from "~/api/queries/models";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";
import { common } from "~/i18n/common";
import { DeleteAliasDialog } from "./delete-alias-dialog";
import { routeSummary } from "./route-summary";

const MODEL_FIELD = "model";

const shadowText = (name: string) => t("models.alias.shadows", "Apps that send `{name}` get this alias, not the model with the same name.", { name });

export function AliasesTab({ org, project, canManage }: { org: string; project: string; canManage: boolean }) {
  const navigate = useNavigate();
  const aliases = useQuery(aliasesQuery(org, project));
  const catalogue = useQuery(catalogueQuery(org, project));
  const models = new Map(catalogue.data?.map((m) => [m.id, m]));
  const [deleting, setDeleting] = useState<AliasView | null>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const open = (alias: string) => void navigate({ to: "/orgs/$org/projects/$project/models/aliases/$alias", params: { org, project, alias } });
  const create = () => void navigate({ to: "/orgs/$org/projects/$project/models/aliases/$alias", params: { org, project, alias: "new" } });
  const list = aliases.data ?? [];
  return (
    <>
      {aliases.isPending ? (
        <List aria-label={t("models.aliases", "Aliases")} items={[]} loading />
      ) : aliases.isError ? (
        <LoadFailed title={t("models.alias.loadFailed", "The aliases couldn't load")} />
      ) : list.length === 0 ? (
        <Empty
          size="lg"
          title={t("models.alias.empty", "No aliases yet")}
          description={tRich("models.alias.emptyHint", "A name like {name} that apps send as {field}; it falls back when a model fails.", {
            name: <Code>{t("models.alias.nameExample", "fast")}</Code>,
            field: <Code>{MODEL_FIELD}</Code>,
          })}
          actions={
            canManage ? (
              <Button variant="primary" onClick={create}>
                {t("models.createAlias", "Create alias")}
              </Button>
            ) : undefined
          }
        />
      ) : (
        <List
          aria-label={t("models.aliases", "Aliases")}
          onOpen={(item) => open(item.id)}
          items={list.map((a) => {
            const removed = a.models.some((m) => !m.target);
            return {
              id: a.id,
              textValue: a.name,
              title: <Text mono>{a.name}</Text>,
              subtitle: (
                <Text variant="small" tone="secondary" truncate>
                  {routeSummary(a.route, models)}
                </Text>
              ),
              meta: (
                <span className="flex items-center gap-6">
                  {removed && <Badge tone="danger">{t("models.alias.modelRemoved", "Model removed")}</Badge>}
                  {a.shadows_model && (
                    <Tooltip content={shadowText(a.name)}>
                      <Badge tone="warning" tabIndex={0}>
                        {t("models.alias.shadowsModel", "Shadows a model")}
                      </Badge>
                    </Tooltip>
                  )}
                </span>
              ),
              date: formatRelative(a.updated_at),
              actions: canManage ? (
                <Menu>
                  <Menu.Trigger>
                    <Button variant="ghost" icon={icons.more} aria-label={t("models.alias.actionsFor", "Actions for {name}", { name: a.name })} size="xs" />
                  </Menu.Trigger>
                  <Menu.Content align="end">
                    <Menu.Item icon={icons.edit} onClick={() => open(a.id)}>
                      {t("models.alias.editRoute", "Edit route")}
                    </Menu.Item>
                    <Menu.Item
                      icon={icons.delete}
                      variant="danger"
                      onClick={() => {
                        setDeleting(a);
                        setDeleteOpen(true);
                      }}
                    >
                      {common.delete()}
                    </Menu.Item>
                  </Menu.Content>
                </Menu>
              ) : undefined,
            };
          })}
        />
      )}
      {deleting && (
        <DialogScope open={deleteOpen}>
          <DeleteAliasDialog org={org} project={project} alias={deleting} open={deleteOpen} onOpenChange={setDeleteOpen} />
        </DialogScope>
      )}
    </>
  );
}
