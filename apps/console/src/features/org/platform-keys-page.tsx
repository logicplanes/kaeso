import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertDialog,
  Button,
  DialogScope,
  Empty,
  FilterRow,
  IconButton,
  icons,
  Input,
  KeyDisplay,
  List,
  Menu,
  SecretReveal,
  StepDialog,
  Text,
  type CodeTab,
  type StepDialogResult,
} from "@logicplanes/kaeso-ui";
import { ScopeTabs, type Scope } from "~/kaeso/scope-tabs";
import { api, data } from "~/api/client";
import type { CreatedKey_OrgKeyView, KeyRotate, OrgKeyView } from "~/api/generated/types";
import { orgQuery, platformKeysQuery } from "~/api/queries/orgs";
import { useApiMutation } from "~/auth/mutation";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { asKeyState } from "~/features/project/keys/party";
import { expiryText } from "~/features/project/keys/expiry";
import { ExpiryPicker, expiryRequest, type Expiry } from "~/kaeso/expiry-picker";
import { rotatedHint, rotationSteps, useRotationChoices } from "~/kaeso/rotate-key";
import { usePageCreateAction } from "~/shell/create-action";
import { toast } from "~/shell/toast";
import { LoadFailed } from "./load-failed";
import { OrgPage } from "./org-page";
import { useNeedsRight } from "~/auth/needs-right";
import { LastUsed } from "~/kaeso/last-used";

export function PlatformKeysPage({ org }: { org: string }) {
  const needs = useNeedsRight(org);
  const rights = useOrgRights(org);
  const [scope, setScope] = useState<Scope>("active");
  const keys = useQuery({ ...platformKeysQuery(org, scope !== "active"), enabled: rights.has("platform_keys.view") });
  const [creating, setCreating] = useState(false);
  const [archiving, setArchiving] = useState<OrgKeyView | null>(null);
  const [archiveOpen, setArchiveOpen] = useState(false);
  const [rotating, setRotating] = useState<OrgKeyView | null>(null);
  const canCreate = rights.has("platform_keys.create");
  const canRevoke = rights.has("platform_keys.revoke");
  usePageCreateAction({
    label: t("org.platformKeys.create", "Create platform key"),
    run: () => setCreating(true),
    disabledReason: canCreate ? undefined : needs("platform_keys.create"),
  });

  const act = useApiMutation({
    mutationFn: async ({ key, action }: { key: OrgKeyView; action: "disable" | "enable" }) => {
      const params = { params: { path: { org, key: key.id } } };
      if (action === "disable") return data(await api.POST("/platform/v1/orgs/{org}/platform-keys/{key}/disable", params));
      return data(await api.POST("/platform/v1/orgs/{org}/platform-keys/{key}/enable", params));
    },
    invalidates: [["orgs", org, "platform-keys"]],
    onSuccess: (k, { action }) =>
      toast(
        action === "disable"
          ? t("org.platformKeys.disabled", "{name} disabled", { name: k.name })
          : t("org.platformKeys.enabled", "{name} enabled", { name: k.name }),
        { action: { label: t("keys.undo", "Undo"), onClick: () => act.mutate({ key: k, action: action === "disable" ? "enable" : "disable" }) } },
      ),
  });
  const archive = (k: OrgKeyView) => {
    setArchiving(k);
    setArchiveOpen(true);
  };

  const fetched = keys.data ?? [];
  const items = (scope === "all" ? fetched : fetched.filter((k) => k.state === scope)).map((k) => {
    const expiry = expiryText(k);
    return {
      id: k.id,
      textValue: k.name,
      title: k.name,
      subtitle: <KeyDisplay redacted={k.redacted} state={asKeyState(k.state)} access="platform" />,
      meta: (
        <Text variant="small" tone={expiry.soon ? "warning" : "secondary"}>
          {expiry.text}
        </Text>
      ),
      date: <LastUsed at={k.last_used_at} />,
      actions:
        k.state === "archived" || !canRevoke ? undefined : (
          <Menu>
            <Menu.Trigger>
              <IconButton icon={icons.more} aria-label={t("org.platformKeys.actions", "Actions for {name}", { name: k.name })} size="xs" />
            </Menu.Trigger>
            <Menu.Content align="end">
              {k.state === "disabled" ? (
                <Menu.Item icon={icons.show} onClick={() => act.mutate({ key: k, action: "enable" })}>
                  {t("org.platformKeys.enable", "Enable")}
                </Menu.Item>
              ) : (
                k.state === "active" && (
                  <Menu.Item icon={icons.hide} onClick={() => act.mutate({ key: k, action: "disable" })}>
                    {t("org.platformKeys.disable", "Disable")}
                  </Menu.Item>
                )
              )}
              {!k.replaced_by && (
                <Menu.Item
                  icon={icons.key}
                  disabled={!canCreate}
                  {...(canCreate ? {} : { description: needs("platform_keys.create") })}
                  onClick={() => setRotating(k)}
                >
                  {t("org.platformKeys.rotate", "Rotate")}
                </Menu.Item>
              )}
              <Menu.Item icon={icons.delete} variant="danger" onClick={() => archive(k)}>
                {common.archive()}
              </Menu.Item>
            </Menu.Content>
          </Menu>
        ),
    };
  });

  return (
    <OrgPage
      org={org}
      title={t("org.platformKeys.title", "Platform keys")}
      hidden={t("org.platformKeys.hidden", "You can't see the platform keys here")}
      right="platform_keys.view"
      description={t("org.platformKeys.description", "Keys for scripts that act with Admin rights and never call models.")}
      actions={
        <Button
          size="sm"
          variant="primary"
          leadingIcon={icons.add}
          disabled={!canCreate}
          {...(canCreate ? {} : { disabledReason: needs("platform_keys.create") })}
          onClick={() => setCreating(true)}
        >
          {t("org.platformKeys.createAction", "Create platform key")}
        </Button>
      }
    >
      <FilterRow tabs={<ScopeTabs label={t("org.platformKeys.title", "Platform keys")} value={scope} onChange={setScope} />} />
      <List
        aria-label={t("org.platformKeys.title", "Platform keys")}
        items={items}
        loading={keys.isPending}
        empty={
          keys.isError ? (
            <LoadFailed title={t("org.platformKeys.loadFailed", "The platform keys couldn't load")} size="md" />
          ) : (
            <Empty
              size="md"
              title={
                scope === "all"
                  ? t("org.platformKeys.noneYet", "No platform keys yet")
                  : scope === "archived"
                    ? t("org.platformKeys.noneArchived", "No archived platform keys")
                    : t("org.platformKeys.noneActive", "No active platform keys")
              }
              {...(scope === "archived"
                ? {}
                : {
                    description:
                      scope === "all"
                        ? t("org.platformKeys.noneYetHint", "Create one for scripts that manage the organization through the Platform API.")
                        : fetched.length > 0
                          ? t("org.platformKeys.inactiveHint", "Disabled and expired keys are under All.")
                          : t("org.platformKeys.noneActiveHint", "Create one for scripts that manage the organization; archived keys are under Archived."),
                  })}
            />
          )
        }
      />
      {creating && <CreatePlatformKeyDialog org={org} onClose={() => setCreating(false)} />}
      {rotating && <RotatePlatformKeyDialog org={org} k={rotating} onClose={() => setRotating(null)} />}
      {archiving && (
        <DialogScope open={archiveOpen}>
          <ArchivePlatformKeyDialog org={org} k={archiving} open={archiveOpen} onOpenChange={setArchiveOpen} />
        </DialogScope>
      )}
    </OrgPage>
  );
}

function ArchivePlatformKeyDialog({ org, k, open, onOpenChange }: { org: string; k: OrgKeyView; open: boolean; onOpenChange: (open: boolean) => void }) {
  const archive = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/platform-keys/{key}/archive", { params: { path: { org, key: k.id } } })),
    invalidates: [["orgs", org, "platform-keys"]],
    onSuccess: (archived) => {
      onOpenChange(false);
      toast(t("org.platformKeys.archived", "{name} archived", { name: archived.name }));
    },
    onError: () => onOpenChange(false),
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("org.platformKeys.archiveConfirm", "Archive {name}?", { name: k.name })}
      description={t("org.platformKeys.archiveHint", "Scripts using it stop authenticating; archiving can't be undone, and its history stays.")}
      confirmLabel={t("org.platformKeys.archiveKey", "Archive key")}
      loading={archive.isPending}
      onConfirm={() => archive.mutateAsync()}
    />
  );
}

function CreatePlatformKeyDialog({ org, onClose }: { org: string; onClose: () => void }) {
  const orgView = useQuery(orgQuery(org));
  const maxDays = orgView.data?.max_key_lifetime_days ?? null;
  const [name, setName] = useState("");
  const [expiry, setExpiry] = useState<Expiry>("90");
  const [date, setDate] = useState<Date | null>(null);
  const [created, setCreated] = useState<CreatedKey_OrgKeyView | null>(null);
  const create = useApiMutation({
    mutationFn: async () =>
      data(await api.POST("/platform/v1/orgs/{org}/platform-keys", { params: { path: { org } }, body: { name: name.trim(), ...expiryRequest(expiry, date) } })),
    invalidates: [["orgs", org, "platform-keys"]],
    onSuccess: setCreated,
  });
  const reveal = usePlatformKeyReveal(org, created, onClose);
  return (
    <>
      <StepDialog
        open
        onOpenChange={(o) => !o && (created ? reveal.done() : onClose())}
        result={reveal.result}
        title={t("org.platformKeys.create", "Create platform key")}
        submitLabel={t("org.platformKeys.createKey", "Create key")}
        submitting={create.isPending}
        onSubmit={() => name.trim() && (expiry !== "date" || date) && create.mutate()}
        steps={[
          {
            id: "name",
            title: t("org.platformKeys.whatUses", "What uses it?"),
            description: t("org.platformKeys.whatUsesHint", "A script or tool, like Terraform; it acts with the Admin role's rights in this organization."),
            blockedReason: name.trim() ? undefined : t("org.platformKeys.nameIt", "Give the key a name"),
            content: (
              <Input aria-label={common.name()} placeholder="Terraform" value={name} onChange={(e) => setName(e.target.value)} maxLength={100} autoFocus />
            ),
          },
          {
            id: "expiry",
            title: t("org.platformKeys.whenExpires", "When does it expire?"),
            description: t("org.platformKeys.whenExpiresHint", "Creating a platform key needs a recent sign-in."),
            blockedReason: expiry === "date" && !date ? t("org.platformKeys.chooseDate", "Choose the expiry date") : undefined,
            content: <ExpiryPicker value={expiry} onChange={setExpiry} date={date} onDateChange={setDate} maxDays={maxDays} scope="org" />,
          },
        ]}
      />
      {reveal.confirm}
    </>
  );
}

function RotatePlatformKeyDialog({ org, k, onClose }: { org: string; k: OrgKeyView; onClose: () => void }) {
  const orgView = useQuery(orgQuery(org));
  const maxDays = orgView.data?.max_key_lifetime_days ?? null;
  const choices = useRotationChoices(maxDays);
  const [created, setCreated] = useState<CreatedKey_OrgKeyView | null>(null);
  const rotate = useApiMutation({
    mutationFn: async (body: KeyRotate) =>
      data(await api.POST("/platform/v1/orgs/{org}/platform-keys/{key}/rotate", { params: { path: { org, key: k.id } }, body })),
    invalidates: [["orgs", org, "platform-keys"]],
    onSuccess: setCreated,
  });
  const reveal = usePlatformKeyReveal(org, created, onClose, {
    title: t("org.platformKeys.rotated", "Platform key rotated"),
    description: rotatedHint(choices.grace),
  });
  return (
    <>
      <StepDialog
        open
        onOpenChange={(o) => !o && (created ? reveal.done() : onClose())}
        result={reveal.result}
        title={t("org.platformKeys.rotateTitle", "Rotate {name}", { name: k.name })}
        submitLabel={t("org.platformKeys.rotateKey", "Rotate key")}
        submitting={rotate.isPending}
        onSubmit={() => choices.ready && rotate.mutate(choices.body)}
        steps={rotationSteps(choices, maxDays, "org")}
      />
      {reveal.confirm}
    </>
  );
}

function platformSnippets(org: string, secret: string): CodeTab[] {
  const url = `${window.location.origin}/platform/v1/orgs/${org}/projects`;
  return [{ label: "curl", lang: "bash", code: `curl ${url} \\\n  -H "Authorization: Bearer ${secret}"` }];
}

function usePlatformKeyReveal(org: string, created: CreatedKey_OrgKeyView | null, onDone: () => void, heading?: { title: string; description: string }) {
  const [copied, setCopied] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const done = () => (copied ? onDone() : setConfirm(true));
  const result: StepDialogResult | undefined = created
    ? {
        size: "lg",
        title: heading?.title ?? t("org.platformKeys.created", "Platform key created"),
        description:
          heading?.description ??
          t("org.platformKeys.createdHint", "Copy the key for {name} now; it's shown once and Kaeso stores only a hash.", { name: created.name }),
        footer: <Button onClick={done}>{common.finish()}</Button>,
        content: (
          <SecretReveal
            secret={created.secret}
            name={created.name}
            baseUrl={window.location.origin}
            snippets={platformSnippets(org, created.secret)}
            onCopied={() => setCopied(true)}
          />
        ),
      }
    : undefined;
  return {
    result,
    done,
    confirm: (
      <AlertDialog
        open={confirm}
        onOpenChange={setConfirm}
        tone="warning"
        title={t("org.platformKeys.copiedConfirm", "Did you copy the key?")}
        description={t("org.platformKeys.copiedConfirmHint", "It can't be shown again; without it, create a new key.")}
        confirmLabel={t("org.platformKeys.closeWithoutCopying", "Close without copying")}
        cancelLabel={t("org.platformKeys.goBack", "Go back")}
        onConfirm={onDone}
      />
    ),
  };
}
