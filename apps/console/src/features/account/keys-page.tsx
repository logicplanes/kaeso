import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AlertDialog, Empty, FilterRow, IconButton, KeyDisplay, List, Menu, Text, icons, type ListGroup } from "@logicplanes/kaeso-ui";
import { ScopeTabs } from "~/kaeso/scope-tabs";
import { toast } from "~/shell/toast";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { KeyView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { useMemberships } from "~/auth/session";
import { asKeyState } from "~/features/project/keys/party";
import { expiryText } from "~/features/project/keys/expiry";
import { projectPath } from "~/shell/pages";
import { SettingsPage } from "~/shell/settings-page";
import { t } from "~/i18n";
import { allPages } from "~/api/paging";

const myKeysQuery = (includeArchived: boolean) => ({
  queryKey: ["me", "keys", { includeArchived }],
  queryFn: (): Promise<KeyView[]> =>
    allPages(async (after) =>
      data(await api.GET("/platform/v1/me/keys", { params: { query: { limit: 100, include_archived: includeArchived, ...(after ? { after } : {}) } } })),
    ),
});

export function AccountKeysPage() {
  const navigate = useNavigate();
  const memberships = useMemberships();
  const [archived, setArchived] = useState(false);
  const keys = useQuery(myKeysQuery(archived));
  const [archiving, setArchiving] = useState<KeyView | null>(null);
  const where = useMemo(() => {
    const map = new Map<string, { org: string; label: string }>();
    for (const m of memberships)
      for (const p of m.projects) map.set(p.id, { org: m.org_id, label: memberships.length > 1 ? `${m.org_name} / ${p.name}` : p.name });
    return map;
  }, [memberships]);
  const readOnly = new Set(memberships.filter((m) => m.org_state === "scheduled_for_deletion").map((m) => m.org_id));

  const act = useApiMutation({
    mutationFn: async ({ key, action }: { key: KeyView; action: "disable" | "enable" | "archive" }) => {
      const params = { params: { path: { key: key.id } } };
      if (action === "disable") return data(await api.POST("/platform/v1/me/keys/{key}/disable", params));
      if (action === "enable") return data(await api.POST("/platform/v1/me/keys/{key}/enable", params));
      return data(await api.POST("/platform/v1/me/keys/{key}/archive", params));
    },
    invalidates: [["me", "keys"], ["orgs"]],
    onSuccess: (k, { action }) => {
      if (action === "archive") setArchiving(null);
      toast(
        action === "disable"
          ? t("account.keyDisabled", "{name} disabled", { name: k.name })
          : action === "enable"
            ? t("account.keyEnabled", "{name} enabled", { name: k.name })
            : t("account.keyArchived", "{name} archived", { name: k.name }),
      );
    },
  });

  const list = keys.data ?? [];
  const groups: ListGroup[] = [...new Set(list.map((k) => k.project_id))].map((projectId) => {
    const items = list.filter((k) => k.project_id === projectId);
    return {
      id: projectId,
      title: where.get(projectId)?.label ?? projectId,
      count: items.length,
      items: items.map((k) => {
        const expiry = expiryText(k);
        return {
          id: k.id,
          textValue: k.name,
          title: k.name,
          subtitle: <KeyDisplay redacted={k.redacted} state={asKeyState(k.state)} copy={false} {...(k.access !== "inference" ? { access: k.access } : {})} />,
          meta: (
            <Text variant="small" tone={expiry.soon ? "warning" : "secondary"}>
              {expiry.text}
            </Text>
          ),
          date: k.last_used_at ? t("account.keyUsed", "Used {when}", { when: formatRelative(k.last_used_at) }) : t("account.keyNeverUsed", "Never used"),
          actions:
            k.state === "archived" || readOnly.has(where.get(k.project_id)?.org ?? "") ? undefined : (
              <Menu>
                <Menu.Trigger>
                  <IconButton icon={icons.more} aria-label={t("account.keyActions", "Actions for {name}", { name: k.name })} size="xs" />
                </Menu.Trigger>
                <Menu.Content align="end">
                  {k.state === "disabled" ? (
                    <Menu.Item icon={icons.show} onClick={() => act.mutate({ key: k, action: "enable" })}>
                      {t("account.enable", "Enable")}
                    </Menu.Item>
                  ) : (
                    k.state === "active" && (
                      <Menu.Item icon={icons.hide} onClick={() => act.mutate({ key: k, action: "disable" })}>
                        {t("account.disable", "Disable")}
                      </Menu.Item>
                    )
                  )}
                  <Menu.Item icon={icons.delete} variant="danger" onClick={() => setArchiving(k)}>
                    {t("common.archive", "Archive")}
                  </Menu.Item>
                </Menu.Content>
              </Menu>
            ),
        };
      }),
    };
  });

  return (
    <SettingsPage
      title={t("account.apiKeys", "API keys")}
      context={t("account.title", "Account")}
      description={t("account.apiKeysHint", "Your own keys in every project; each opens in its project.")}
    >
      <FilterRow tabs={<ScopeTabs label={t("account.yourKeys", "Your keys")} all={archived} onChange={setArchived} />} />
      <List
        aria-label={t("account.yourKeys", "Your keys")}
        groups={groups}
        loading={keys.isPending}
        onOpen={(item) => {
          const key = list.find((k) => k.id === item.id);
          const org = key && where.get(key.project_id)?.org;
          if (key && org) void navigate({ href: `${projectPath(org, key.project_id)}/keys?key=${key.id}` });
        }}
        empty={
          <Empty
            size="md"
            title={archived ? t("account.noKeys", "No keys yet") : t("account.noActiveKeys", "No active keys")}
            description={
              archived
                ? t("account.noKeysHint", "Create keys on a project's API keys page; they're listed here across projects.")
                : t("account.noActiveKeysHint", "Archived keys are under All; create new ones on a project's API keys page.")
            }
          />
        }
      />
      <AlertDialog
        open={!!archiving}
        onOpenChange={(o) => !o && setArchiving(null)}
        tone="danger"
        title={archiving ? t("account.archiveKeyConfirm", "Archive {name}?", { name: archiving.name }) : t("account.archiveKeyConfirmAny", "Archive this key?")}
        description={t(
          "account.archiveKeyConfirmHint",
          "Applications using it stop authenticating with the gateway. Archiving can't be undone; its history stays.",
        )}
        confirmLabel={t("account.archiveKey", "Archive key")}
        loading={act.isPending}
        onConfirm={() => {
          if (archiving) act.mutate({ key: archiving, action: "archive" });
        }}
      />
    </SettingsPage>
  );
}
