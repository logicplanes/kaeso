import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertDialog,
  Badge,
  Button,
  Checkbox,
  CheckboxGroup,
  Code,
  DialogScope,
  Empty,
  Field,
  Input,
  KeyValue,
  List,
  Menu,
  SecretReveal,
  Select,
  Sheet,
  StepDialog,
  Text,
  icons,
  type StepDialogResult,
} from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { WebhookDeliveryView, WebhookFormat, WebhookView } from "~/api/generated/types";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { kindName } from "~/kaeso/notification-kinds";
import { usePageCreateAction } from "~/shell/create-action";
import { toast } from "~/shell/toast";
import { LoadFailed } from "./load-failed";
import { OrgPage } from "./org-page";

/** The notices a webhook can receive, in the order of the notification settings. */
export const WEBHOOK_KINDS = [
  "budget_alerts",
  "limit_reached",
  "key_expiry",
  "content_logging",
  "mcp_servers",
  "org_security",
  "org_members",
  "provider_keys",
  "model_changes",
] as const;

const WEBHOOKS = (org: string) => ["orgs", org, "webhooks"];

const webhooksQuery = (org: string) => ({
  queryKey: WEBHOOKS(org),
  queryFn: async (): Promise<WebhookView[]> => data(await api.GET("/platform/v1/orgs/{org}/webhooks", { params: { path: { org } } })).data,
});

const deliveriesQuery = (org: string, webhook: string) => ({
  queryKey: [...WEBHOOKS(org), webhook, "deliveries"],
  queryFn: async (): Promise<WebhookDeliveryView[]> =>
    data(await api.GET("/platform/v1/orgs/{org}/webhooks/{webhook}/deliveries", { params: { path: { org, webhook } } })).data,
});

const formatLabel = (f: WebhookFormat): string => (f === "slack" ? t("org.webhooks.formatSlack", "Slack") : t("org.webhooks.formatJson", "JSON"));

function stateBadge(w: WebhookView) {
  if (w.enabled) return <Badge tone="success">{t("org.webhooks.on", "On")}</Badge>;
  if (w.disabled_reason === "failures") return <Badge tone="danger">{t("org.webhooks.turnedOff", "Off after failures")}</Badge>;
  return <Badge>{t("org.webhooks.off", "Off")}</Badge>;
}

function deliveryStatus(d: WebhookDeliveryView): string {
  if (d.status === "delivered") return t("org.webhooks.delivered", "Delivered");
  if (d.status === "failed") return t("org.webhooks.failed", "Failed");
  return t("org.webhooks.pending", "Waiting to try again");
}

export function WebhooksPage({ org }: { org: string }) {
  const needs = useNeedsRight(org);
  const rights = useOrgRights(org);
  const hooks = useQuery({ ...webhooksQuery(org), enabled: rights.has("webhooks.view") });
  const canManage = rights.has("webhooks.manage");
  const [creating, setCreating] = useState(false);
  const [open, setOpen] = useState<string | undefined>();
  const [deleting, setDeleting] = useState<WebhookView | null>(null);
  const [rotating, setRotating] = useState<WebhookView | null>(null);
  usePageCreateAction({
    label: t("org.webhooks.add", "Add webhook"),
    run: () => setCreating(true),
    disabledReason: canManage ? undefined : needs("webhooks.manage"),
  });
  const toggle = useApiMutation({
    mutationFn: async (w: WebhookView) =>
      data(await api.PATCH("/platform/v1/orgs/{org}/webhooks/{webhook}", { params: { path: { org, webhook: w.id } }, body: { enabled: !w.enabled } })),
    invalidates: [WEBHOOKS(org)],
    onSuccess: (w) =>
      toast(
        w.enabled ? t("org.webhooks.turnedOn", "{name} turned on", { name: w.name }) : t("org.webhooks.turnedOffToast", "{name} turned off", { name: w.name }),
      ),
  });
  const test = useTest(org);

  const items = (hooks.data ?? []).map((w) => ({
    id: w.id,
    textValue: w.name,
    title: w.name,
    subtitle: (
      <Text variant="small" tone="secondary" mono truncate>
        {w.url}
      </Text>
    ),
    meta: stateBadge(w),
    date: (
      <Text variant="small" tone={w.last_error ? "danger" : "secondary"}>
        {w.last_attempt_at
          ? w.last_error
            ? t("org.webhooks.lastFailed", "Last try failed")
            : t("org.webhooks.lastDelivered", "Last delivered {date}", { date: formatDate(w.last_attempt_at) })
          : t("org.webhooks.nothingYet", "Nothing sent yet")}
      </Text>
    ),
    actions: canManage ? (
      <Menu>
        <Menu.Trigger>
          <Button variant="ghost" icon={icons.more} aria-label={t("org.webhooks.actions", "Actions for {name}", { name: w.name })} size="xs" />
        </Menu.Trigger>
        <Menu.Content align="end">
          <Menu.Item
            icon={icons.send}
            disabled={!w.enabled}
            {...(w.enabled ? {} : { description: t("org.webhooks.testOff", "Turn it on to send a test.") })}
            onClick={() => test.mutate(w)}
          >
            {t("org.webhooks.sendTest", "Send test")}
          </Menu.Item>
          <Menu.Item icon={w.enabled ? icons.hide : icons.show} onClick={() => toggle.mutate(w)}>
            {w.enabled ? t("org.webhooks.turnOff", "Turn off") : t("org.webhooks.turnOn", "Turn on")}
          </Menu.Item>
          <Menu.Item icon={icons.key} onClick={() => setRotating(w)}>
            {t("org.webhooks.rotate", "Rotate secret")}
          </Menu.Item>
          <Menu.Item icon={icons.delete} variant="danger" onClick={() => setDeleting(w)}>
            {common.delete()}
          </Menu.Item>
        </Menu.Content>
      </Menu>
    ) : undefined,
  }));

  return (
    <OrgPage
      org={org}
      title={t("org.webhooks.title", "Webhooks")}
      hidden={t("org.webhooks.hidden", "You can't see the webhooks here")}
      right="webhooks.view"
      description={t("org.webhooks.description", "Send the organization's notices, signed, to chat and on-call tools.")}
      actions={
        <Button
          size="sm"
          variant="primary"
          leadingIcon={icons.add}
          disabled={!canManage}
          {...(canManage ? {} : { disabledReason: needs("webhooks.manage") })}
          onClick={() => setCreating(true)}
        >
          {t("org.webhooks.add", "Add webhook")}
        </Button>
      }
    >
      <List
        aria-label={t("org.webhooks.title", "Webhooks")}
        items={items}
        loading={hooks.isPending}
        onOpen={(item) => setOpen(item.id)}
        empty={
          hooks.isError ? (
            <LoadFailed title={t("org.webhooks.loadFailed", "The webhooks couldn't load")} size="md" />
          ) : (
            <Empty
              size="md"
              title={t("org.webhooks.none", "No webhooks yet")}
              description={t("org.webhooks.noneHint", "Add one to post budget alerts, refused provider keys and more to a URL of yours.")}
            />
          )
        }
      />
      {creating && <CreateWebhookDialog org={org} onClose={() => setCreating(false)} />}
      {rotating && <RotateDialog org={org} webhook={rotating} onClose={() => setRotating(null)} />}
      <DialogScope open={!!deleting}>{deleting && <DeleteDialog org={org} webhook={deleting} onClose={() => setDeleting(null)} />}</DialogScope>
      <WebhookSheet org={org} webhook={hooks.data?.find((w) => w.id === open)} onClose={() => setOpen(undefined)} />
    </OrgPage>
  );
}

function useTest(org: string) {
  return useApiMutation({
    mutationFn: async (w: WebhookView) => data(await api.POST("/platform/v1/orgs/{org}/webhooks/{webhook}/test", { params: { path: { org, webhook: w.id } } })),
    invalidates: [WEBHOOKS(org)],
    onSuccess: (d) =>
      d.status === "delivered"
        ? toast.success(t("org.webhooks.testDelivered", "Test delivered"))
        : toast.error(t("org.webhooks.testFailed", "The test didn't arrive; its delivery below says why.")),
  });
}

function WebhookSheet({ org, webhook, onClose }: { org: string; webhook: WebhookView | undefined; onClose: () => void }) {
  return webhook ? <WebhookDetails org={org} webhook={webhook} onClose={onClose} /> : null;
}

function WebhookDetails({ org, webhook, onClose }: { org: string; webhook: WebhookView; onClose: () => void }) {
  const projects = useQuery(orgProjectsQuery(org));
  return (
    <Sheet open onOpenChange={(o) => !o && onClose()} modal={false} size="md" title={webhook.name}>
      {
        <div className="flex flex-col gap-24">
          <KeyValue
            items={[
              { label: t("org.webhooks.url", "URL"), value: webhook.url, mono: true, wrap: true, copy: webhook.url },
              { label: t("org.webhooks.format", "Format"), value: formatLabel(webhook.format) },
              { label: t("org.webhooks.state", "State"), value: stateBadge(webhook) },
              {
                label: t("org.webhooks.project", "Events of"),
                value: webhook.project
                  ? (projects.data?.find((p) => p.id === webhook.project)?.name ?? webhook.project)
                  : t("org.webhooks.wholeOrg", "The whole organization"),
              },
              { label: t("org.webhooks.receives", "Receives"), value: webhook.kinds.map((k) => kindName(k, k)).join(", "), wrap: true },
            ]}
          />
          <Deliveries key={webhook.id} org={org} webhook={webhook.id} />
        </div>
      }
    </Sheet>
  );
}

function Deliveries({ org, webhook }: { org: string; webhook: string }) {
  const list = useQuery(deliveriesQuery(org, webhook));
  return (
    <div className="flex flex-col gap-8">
      <Text variant="body" weight="medium">
        {t("org.webhooks.lastDeliveries", "Last deliveries")}
      </Text>
      <List
        aria-label={t("org.webhooks.lastDeliveries", "Last deliveries")}
        density="compact"
        loading={list.isPending}
        items={(list.data ?? []).map((d) => ({
          id: d.id,
          textValue: d.type,
          title: <Code>{d.type}</Code>,
          subtitle:
            d.last_error ??
            t("org.webhooks.tries", "{count, plural, one {# try} other {# tries}}", {
              count: d.attempts,
            }),
          meta: (
            <Badge tone={d.status === "delivered" ? "success" : d.status === "failed" ? "danger" : "warning"}>
              {deliveryStatus(d)}
              {d.last_status ? ` · ${d.last_status}` : ""}
            </Badge>
          ),
          date: <Text variant="small">{formatDate(d.created_at)}</Text>,
        }))}
        empty={
          list.isError ? (
            <LoadFailed title={t("org.webhooks.deliveriesFailed", "The deliveries couldn't load")} size="md" />
          ) : (
            <Empty size="compact" title={t("org.webhooks.noDeliveries", "No deliveries in the last 7 days")} />
          )
        }
      />
    </div>
  );
}

function CreateWebhookDialog({ org, onClose }: { org: string; onClose: () => void }) {
  const projects = useQuery(orgProjectsQuery(org));
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [format, setFormat] = useState<WebhookFormat>("json");
  const [kinds, setKinds] = useState<string[]>(["budget_alerts", "provider_keys"]);
  const [project, setProject] = useState("");
  const [secret, setSecret] = useState<{ name: string; secret: string } | null>(null);
  const create = useApiMutation({
    mutationFn: async () =>
      data(
        await api.POST("/platform/v1/orgs/{org}/webhooks", {
          params: { path: { org } },
          body: { name: name.trim(), url: url.trim(), format, kinds, ...(project ? { project } : {}) },
        }),
      ),
    invalidates: [WEBHOOKS(org)],
    onSuccess: (w) => setSecret({ name: w.name, secret: w.secret }),
  });
  const result: StepDialogResult | undefined = secret
    ? {
        size: "lg",
        title: t("org.webhooks.created", "Webhook added"),
        description: t("org.webhooks.createdHint", "Copy the signing secret for {name} now; it's shown only once.", {
          name: secret.name,
        }),
        footer: <Button onClick={onClose}>{common.finish()}</Button>,
        content: <SecretReveal secret={secret.secret} name={secret.name} snippets={false} baseUrl={window.location.origin} />,
      }
    : undefined;
  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      result={result}
      title={t("org.webhooks.add", "Add webhook")}
      submitLabel={t("org.webhooks.addWebhook", "Add webhook")}
      submitting={create.isPending}
      onSubmit={() => name.trim() && url.trim() && kinds.length > 0 && create.mutate()}
      steps={[
        {
          id: "where",
          title: t("org.webhooks.where", "Where do events go?"),
          description: t("org.webhooks.whereHint", "An https address, or http to a network this install allows."),
          blockedReason: !name.trim()
            ? t("org.webhooks.nameIt", "Give the webhook a name")
            : !url.trim()
              ? t("org.webhooks.urlNeeded", "Enter the receiver's URL")
              : undefined,
          content: (
            <div className="flex flex-col gap-16">
              <Input
                label={common.name()}
                placeholder={t("org.webhooks.namePlaceholder", "On-call")}
                value={name}
                onChange={(e) => setName(e.target.value)}
                maxLength={100}
                autoFocus
              />
              <Input
                label={t("org.webhooks.url", "URL")}
                placeholder={t("org.webhooks.urlPlaceholder", "https://hooks.example.com/kaeso")}
                value={url}
                onChange={(e) => setUrl(e.target.value)}
              />
              <Select
                label={t("org.webhooks.format", "Format")}
                fullWidth
                value={format}
                onValueChange={(v) => setFormat(v as WebhookFormat)}
                options={[
                  { value: "json", label: t("org.webhooks.formatJsonHint", "JSON: Kaeso's event, for scripts and on-call tools") },
                  { value: "slack", label: t("org.webhooks.formatSlackHint", "Slack: one line of text, for chat") },
                ]}
              />
            </div>
          ),
        },
        {
          id: "what",
          title: t("org.webhooks.what", "What does it receive?"),
          description: t("org.webhooks.whatHint", "Whatever people chose for their own emails, the webhook gets these."),
          blockedReason: kinds.length === 0 ? t("org.webhooks.chooseKinds", "Choose at least one notice") : undefined,
          content: (
            <div className="flex flex-col gap-16">
              <Field label={t("org.webhooks.receives", "Receives")}>
                <CheckboxGroup aria-label={t("org.webhooks.receives", "Receives")} value={kinds} onValueChange={setKinds}>
                  {WEBHOOK_KINDS.map((k) => (
                    <Checkbox key={k} value={k} label={kindName(k, k)} />
                  ))}
                </CheckboxGroup>
              </Field>
              <Select
                label={t("org.webhooks.project", "Events of")}
                fullWidth
                value={project}
                onValueChange={setProject}
                options={[
                  { value: "", label: t("org.webhooks.wholeOrg", "The whole organization") },
                  ...(projects.data ?? []).map((p) => ({ value: p.id, label: p.name })),
                ]}
              />
            </div>
          ),
        },
      ]}
    />
  );
}

function RotateDialog({ org, webhook, onClose }: { org: string; webhook: WebhookView; onClose: () => void }) {
  const [secret, setSecret] = useState<string | null>(null);
  const rotate = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/webhooks/{webhook}/secret", { params: { path: { org, webhook: webhook.id } } })),
    invalidates: [WEBHOOKS(org)],
    onSuccess: (r) => setSecret(r.secret),
  });
  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      result={
        secret
          ? {
              size: "lg",
              title: t("org.webhooks.rotated", "New signing secret"),
              description: t("org.webhooks.rotatedHint", "Deliveries are signed with it from now on; update your receiver."),
              footer: <Button onClick={onClose}>{common.finish()}</Button>,
              content: <SecretReveal secret={secret} name={webhook.name} snippets={false} baseUrl={window.location.origin} />,
            }
          : undefined
      }
      title={t("org.webhooks.rotateTitle", "Rotate the secret of {name}", { name: webhook.name })}
      submitLabel={t("org.webhooks.rotate", "Rotate secret")}
      submitting={rotate.isPending}
      onSubmit={() => rotate.mutate()}
      steps={[
        {
          id: "confirm",
          title: t("org.webhooks.rotateConfirm", "Replace the signing secret?"),
          description: t("org.webhooks.rotateConfirmHint", "The old secret stops signing at once, so its receiver rejects deliveries until updated."),
          content: null,
        },
      ]}
    />
  );
}

function DeleteDialog({ org, webhook, onClose }: { org: string; webhook: WebhookView; onClose: () => void }) {
  const remove = useApiMutation({
    mutationFn: async () => data(await api.DELETE("/platform/v1/orgs/{org}/webhooks/{webhook}", { params: { path: { org, webhook: webhook.id } } })),
    invalidates: [WEBHOOKS(org)],
    onSuccess: () => {
      onClose();
      toast(t("org.webhooks.deleted", "{name} deleted", { name: webhook.name }));
    },
    onError: onClose,
  });
  return (
    <AlertDialog
      open
      onOpenChange={(o) => !o && onClose()}
      tone="danger"
      title={t("org.webhooks.deleteConfirm", "Delete {name}?", { name: webhook.name })}
      description={t("org.webhooks.deleteHint", "Events no longer go to it, and its deliveries are gone.")}
      confirmLabel={t("org.webhooks.deleteWebhook", "Delete webhook")}
      loading={remove.isPending}
      onConfirm={() => remove.mutateAsync()}
    />
  );
}
