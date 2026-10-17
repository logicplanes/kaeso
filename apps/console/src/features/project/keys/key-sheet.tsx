import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertDialog, Button, Empty, IconButton, KeyDisplay, KeyValue, Menu, Sheet, Skeleton, icons } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { formatDate, formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { KeyView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { useSession } from "~/auth/session";
import { accessLabel } from "~/format/labels";
import { LimitsSection } from "~/kaeso/limits-section";
import { ModelAccessSection } from "~/kaeso/model-access-section";
import { expiryDetail } from "./expiry";
import { asKeyState, usePartyName } from "./party";
import { useRememberRecent } from "~/shell/recent";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { common } from "~/i18n/common";

const keyQuery = (org: string, project: string, key: string) => ({
  queryKey: ["orgs", org, "projects", project, "keys", "one", key],
  queryFn: async (): Promise<KeyView> =>
    data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/keys/{key}", { params: { path: { org, project, key } } })),
});

export function KeySheet({ org, project, keyId, onClose }: { org: string; project: string; keyId: string | undefined; onClose: () => void }) {
  return (
    <Sheet
      open={!!keyId}
      onOpenChange={(o) => !o && onClose()}
      modal={false}
      size="md"
      title={keyId ? <SheetTitle org={org} project={project} keyId={keyId} /> : ""}
    >
      {keyId && <KeyBody key={keyId} org={org} project={project} keyId={keyId} onClose={onClose} />}
    </Sheet>
  );
}

function SheetTitle({ org, project, keyId }: { org: string; project: string; keyId: string }) {
  const key = useQuery(keyQuery(org, project, keyId));
  return <>{key.data?.name ?? t("keys.key", "Key")}</>;
}

function KeyBody({ org, project, keyId, onClose }: { org: string; project: string; keyId: string; onClose: () => void }) {
  const needs = useNeedsRight(org, project);
  const session = useSession();
  const rights = useProjectRights(org, project);
  const queryClient = useQueryClient();
  const key = useQuery(keyQuery(org, project, keyId));
  const partyName = usePartyName(org, project);
  useRememberRecent(key.data ? { kind: "key", id: keyId, label: key.data.name, href: `/orgs/${org}/projects/${project}/keys?key=${keyId}` } : null);
  const [archiving, setArchiving] = useState(false);

  const own = key.data?.principal.kind === "user" && key.data.principal.id === session.user.id;
  const canManage = rights.has("keys.all.manage") || (own && rights.has("keys.own.manage"));
  const viaMe = !rights.has("keys.all.manage") && own;

  const act = async (action: "disable" | "enable" | "archive"): Promise<KeyView> => {
    const own = { params: { path: { key: keyId } } };
    const any = { params: { path: { org, project, key: keyId } } };
    if (action === "disable")
      return data(
        viaMe
          ? await api.POST("/platform/v1/me/keys/{key}/disable", own)
          : await api.POST("/platform/v1/orgs/{org}/projects/{project}/keys/{key}/disable", any),
      );
    if (action === "enable")
      return data(
        viaMe ? await api.POST("/platform/v1/me/keys/{key}/enable", own) : await api.POST("/platform/v1/orgs/{org}/projects/{project}/keys/{key}/enable", any),
      );
    return data(
      viaMe ? await api.POST("/platform/v1/me/keys/{key}/archive", own) : await api.POST("/platform/v1/orgs/{org}/projects/{project}/keys/{key}/archive", any),
    );
  };

  const toggle = useApiMutation({
    mutationFn: (action: "disable" | "enable") => act(action),
    onMutate: (action) => {
      queryClient.setQueryData<KeyView>(keyQuery(org, project, keyId).queryKey, (k) => (k ? { ...k, state: action === "disable" ? "disabled" : "active" } : k));
    },
    invalidates: [
      ["orgs", org, "projects", project, "keys"],
      ["me", "keys"],
    ],
    onError: () => void queryClient.invalidateQueries({ queryKey: keyQuery(org, project, keyId).queryKey }),
    onSuccess: (_k, action) =>
      toast(action === "disable" ? t("keys.disabled", "Key disabled") : t("keys.enabled", "Key enabled"), {
        action: { label: t("keys.undo", "Undo"), onClick: () => toggle.mutate(action === "disable" ? "enable" : "disable") },
      }),
  });
  const archive = useApiMutation({
    mutationFn: () => act("archive"),
    invalidates: [
      ["orgs", org, "projects", project, "keys"],
      ["me", "keys"],
    ],
    onSuccess: () => {
      toast(t("keys.archived", "Key archived"));
      onClose();
    },
  });

  if (key.isPending) return <Skeleton className="w-full" />;
  if (key.isError) {
    return (
      <Empty
        title={
          isApiError(key.error) && key.error.type === "not_found"
            ? t("keys.notFound", "This key doesn't exist or was removed.")
            : t("keys.loadFailed", "Couldn't load this key.")
        }
        actions={<Button onClick={onClose}>{t("keys.backToKeys", "Back to keys")}</Button>}
      />
    );
  }
  const k = key.data;
  const reason = canManage ? undefined : needs(own ? "keys.own.manage" : "keys.all.manage");

  return (
    <div className="flex flex-col gap-24">
      <div className="flex items-center justify-between gap-12">
        <KeyDisplay redacted={k.redacted} state={asKeyState(k.state)} copy />
        {k.state !== "archived" && (
          <Menu>
            <Menu.Trigger>
              <IconButton
                icon={icons.more}
                aria-label={t("keys.actions", "Key actions")}
                disabled={!canManage}
                {...(reason ? { disabledReason: reason } : {})}
              />
            </Menu.Trigger>
            <Menu.Content align="end">
              {k.state === "disabled" ? (
                <Menu.Item icon={icons.show} onClick={() => toggle.mutate("enable")}>
                  {t("keys.enable", "Enable")}
                </Menu.Item>
              ) : (
                k.state === "active" && (
                  <Menu.Item icon={icons.hide} onClick={() => toggle.mutate("disable")}>
                    {t("keys.disable", "Disable")}
                  </Menu.Item>
                )
              )}
              <Menu.Item icon={icons.delete} variant="danger" onClick={() => setArchiving(true)}>
                {common.archive()}
              </Menu.Item>
            </Menu.Content>
          </Menu>
        )}
      </div>
      <KeyValue
        items={[
          { label: t("keys.for", "For"), value: partyName(k.principal) },
          { label: t("keys.access", "Access"), value: accessLabel(k.access) },
          { label: common.created(), value: t("keys.createdBy", "{date} by {name}", { date: formatDate(k.created_at), name: partyName(k.created_by) }) },
          { label: common.expires(), value: k.expires_at ? expiryDetail(k.expires_at) : common.never() },
          { label: common.lastUsed(), value: k.last_used_at ? formatRelative(k.last_used_at) : common.never() },
          { label: t("keys.id", "Id"), value: k.id, copy: k.id },
        ]}
      />
      {rights.has("limits.view") && (
        <ModelAccessSection
          org={org}
          project={project}
          subject={{
            kind: "key",
            id: k.id,
            owner: { kind: k.principal.kind === "service_account" ? "service_account" : "user", id: k.principal.id ?? "" },
          }}
          canEdit={rights.has("limits.manage")}
        />
      )}
      {rights.has("limits.view") && (
        <LimitsSection
          org={org}
          project={project}
          filter={{ key: k.id }}
          target={{ level: "key", key: k.id }}
          owner={{ kind: k.principal.kind === "service_account" ? "service_account" : "user", id: k.principal.id ?? "" }}
          canEdit={rights.has("limits.manage")}
          empty={t("keys.noLimits", "No limits on this key; its owner's and the project's limits apply.")}
        />
      )}
      <AlertDialog
        open={archiving}
        onOpenChange={setArchiving}
        tone="danger"
        title={t("keys.archiveTitle", "Archive {name}?", { name: k.name })}
        description={t("keys.archiveHint", "Applications using it stop authenticating; archiving can't be undone, and its history stays.")}
        confirmLabel={t("keys.archiveKey", "Archive key")}
        loading={archive.isPending}
        onConfirm={() => archive.mutate()}
      />
    </div>
  );
}
