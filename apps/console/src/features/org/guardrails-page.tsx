import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertDialog, Button, DialogScope, Empty, List, Menu, Text, icons } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { GuardrailView, GuardrailWithSecret } from "~/api/generated/types";
import { guardrailsKey, guardrailsQuery } from "~/api/queries/guardrails";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { usePageCreateAction } from "~/shell/create-action";
import { toast } from "~/shell/toast";
import { GuardrailDialog, SecretDialog } from "./guardrail-dialog";
import { GuardrailTestDialog } from "./guardrail-test-dialog";
import { LoadFailed } from "./load-failed";
import { OrgPage } from "./org-page";

export function phaseWords(g: Pick<GuardrailView, "phases" | "streams">): string {
  const request = g.phases.includes("request");
  const reply = g.phases.includes("reply");
  if (request && reply) {
    return g.streams === "hold"
      ? t("org.guardrails.phases.bothHold", "Requests and replies, streams held")
      : t("org.guardrails.phases.both", "Requests and replies");
  }
  if (reply) {
    return g.streams === "hold" ? t("org.guardrails.phases.replyHold", "Replies, streams held") : t("org.guardrails.phases.reply", "Replies");
  }
  return t("org.guardrails.phases.request", "Requests");
}

export function GuardrailsPage({ org }: { org: string }) {
  const needs = useNeedsRight(org);
  const rights = useOrgRights(org);
  const canView = rights.has("guardrails.view");
  const canManage = rights.has("guardrails.manage");
  const guardrails = useQuery({ ...guardrailsQuery(org), enabled: canView });
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<GuardrailView | null>(null);
  const [testing, setTesting] = useState<GuardrailView | null>(null);
  const [deleting, setDeleting] = useState<GuardrailView | null>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [secret, setSecret] = useState<GuardrailWithSecret | null>(null);
  const manageReason = canManage ? undefined : needs("guardrails.manage");
  usePageCreateAction(canView ? { label: t("org.guardrails.add", "Add guardrail"), run: () => setAdding(true), disabledReason: manageReason } : null);

  const rotate = useApiMutation({
    mutationFn: async (g: GuardrailView) =>
      data(await api.POST("/platform/v1/orgs/{org}/guardrails/{guardrail}/secret", { params: { path: { org, guardrail: g.id } } })),
    invalidates: [guardrailsKey(org)],
    onSuccess: setSecret,
  });

  const items = (guardrails.data ?? []).map((g) => ({
    id: g.id,
    textValue: g.name,
    title: g.name,
    subtitle: (
      <Text variant="small" tone="secondary" truncate>
        {g.url}
      </Text>
    ),
    meta: (
      <Text variant="small" tone="secondary">
        {phaseWords(g)}
      </Text>
    ),
    date: (
      <Text variant="small" tone="secondary">
        {t("org.guardrails.projectCount", "{count, plural, =0 {No projects} one {# project} other {# projects}}", { count: g.projects.length })}
      </Text>
    ),
    actions: (
      <Menu>
        <Menu.Trigger>
          <Button variant="ghost" icon={icons.more} aria-label={t("org.guardrails.actions", "Actions for {name}", { name: g.name })} size="xs" />
        </Menu.Trigger>
        <Menu.Content align="end">
          <Menu.Item icon={icons.edit} disabled={!canManage} {...(manageReason ? { description: manageReason } : {})} onClick={() => setEditing(g)}>
            {common.edit()}
          </Menu.Item>
          <Menu.Item icon={icons.send} disabled={!canManage} {...(manageReason ? { description: manageReason } : {})} onClick={() => setTesting(g)}>
            {t("org.guardrails.test", "Test")}
          </Menu.Item>
          <Menu.Item icon={icons.key} disabled={!canManage} {...(manageReason ? { description: manageReason } : {})} onClick={() => rotate.mutate(g)}>
            {t("org.guardrails.newSecret", "New secret")}
          </Menu.Item>
          <Menu.Item
            icon={icons.delete}
            variant="danger"
            disabled={!canManage}
            {...(manageReason ? { description: manageReason } : {})}
            onClick={() => {
              setDeleting(g);
              setDeleteOpen(true);
            }}
          >
            {common.delete()}
          </Menu.Item>
        </Menu.Content>
      </Menu>
    ),
  }));

  return (
    <OrgPage
      org={org}
      title={t("org.guardrails.title", "Guardrails")}
      hidden={t("org.guardrails.hidden", "You can't see the guardrails here")}
      right="guardrails.view"
      description={t("org.guardrails.description", "Services that check requests before a model gets them, and replies.")}
      actions={
        <Button
          size="sm"
          variant="primary"
          leadingIcon={icons.add}
          disabled={!canManage}
          {...(manageReason ? { disabledReason: manageReason } : {})}
          onClick={() => setAdding(true)}
        >
          {t("org.guardrails.add", "Add guardrail")}
        </Button>
      }
    >
      <List
        aria-label={t("org.guardrails.title", "Guardrails")}
        items={items}
        loading={guardrails.isPending}
        empty={
          guardrails.isError ? (
            <LoadFailed title={t("org.guardrails.loadFailed", "The guardrails couldn't load")} size="md" />
          ) : (
            <Empty
              size="md"
              title={t("org.guardrails.none", "No guardrails yet")}
              description={t("org.guardrails.noneHint", "Add one, then choose the projects it checks under each project's settings.")}
            />
          )
        }
      />
      {adding && <GuardrailDialog org={org} onClose={() => setAdding(false)} />}
      {editing && <GuardrailDialog org={org} guardrail={editing} onClose={() => setEditing(null)} />}
      {testing && <GuardrailTestDialog org={org} guardrail={testing} onClose={() => setTesting(null)} />}
      {secret && <SecretDialog created={secret} onClose={() => setSecret(null)} />}
      {deleting && (
        <DialogScope open={deleteOpen}>
          <DeleteGuardrailDialog org={org} guardrail={deleting} open={deleteOpen} onOpenChange={setDeleteOpen} />
        </DialogScope>
      )}
    </OrgPage>
  );
}

function DeleteGuardrailDialog({
  org,
  guardrail,
  open,
  onOpenChange,
}: {
  org: string;
  guardrail: GuardrailView;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const remove = useApiMutation({
    mutationFn: async () => data(await api.DELETE("/platform/v1/orgs/{org}/guardrails/{guardrail}", { params: { path: { org, guardrail: guardrail.id } } })),
    invalidates: [guardrailsKey(org), ["orgs", org, "projects"]],
    onSuccess: () => {
      onOpenChange(false);
      toast.success(t("org.guardrails.deleted", "{name} deleted", { name: guardrail.name }));
    },
    onError: () => onOpenChange(false),
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("org.guardrails.deleteConfirm", "Delete {name}?", { name: guardrail.name })}
      description={t("org.guardrails.deleteHint", "Projects that use it stop being checked by it at once.")}
      confirmLabel={t("org.guardrails.deleteButton", "Delete guardrail")}
      loading={remove.isPending}
      onConfirm={() => remove.mutateAsync()}
    />
  );
}
