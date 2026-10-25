import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AlertDialog, Empty, FilterRow, IconButton, KeyDisplay, List, Menu, Text, icons, type ListGroup } from "@logicplanes/kaeso-ui";
import { ScopeTabs, type Scope } from "~/kaeso/scope-tabs";
import { toast } from "~/shell/toast";
import { api, data } from "~/api/client";
import type { KeyView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { useMemberships } from "~/auth/session";
import { LoadFailed } from "~/features/org/load-failed";
import { asKeyState } from "~/features/project/keys/party";
import { expiryText } from "~/features/project/keys/expiry";
import { projectPath } from "~/shell/pages";
import { SettingsPage } from "~/shell/settings-page";
import { t } from "~/i18n";
import { allPages } from "~/api/paging";
import { LastUsed } from "~/kaeso/last-used";

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
  const [scope, setScope] = useState<Scope>("active");
  const keys = useQuery(myKeysQuery(scope !== "active"));
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
    invalidates: ({ key }) => {
      const org = where.get(key.project_id)?.org;
      return [["me", "keys"], ...(org ? [["orgs", org, "projects", key.project_id, "keys"]] : [])];
    },
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

  const fetched = keys.data ?? [];
  const list = scope === "all" ? fetched : fetched.filter((k) => k.state === scope);
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
          date: <LastUsed at={k.last_used_at} />,
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
      <FilterRow tabs={<ScopeTabs label={t("account.yourKeys", "Your keys")} value={scope} onChange={setScope} />} />
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
          keys.isError ? (
            <LoadFailed title={t("account.keysFailed", "Your keys couldn't load")} size="md" />
          ) : (
            <Empty
              size="md"
              title={
                scope === "all"
                  ? t("account.noKeys", "No keys yet")
                  : scope === "archived"
                    ? t("account.noArchivedKeys", "No archived keys")
                    : t("account.noActiveKeys", "No active keys")
              }
              {...(scope === "archived"
                ? {}
                : {
                    description:
                      scope === "all"
                        ? t("account.noKeysHint", "Create keys on a project's API keys page; they're listed here across projects.")
                        : fetched.length > 0
                          ? t("account.inactiveKeysHint", "Disabled and expired keys are under All.")
                          : t("account.noActiveKeysHint", "Archived keys are under Archived; create new ones on a project's API keys page."),
                  })}
            />
          )
        }
      />
      <AlertDialog
        open={!!archiving}
        onOpenChange={(o) => !o && setArchiving(null)}
        tone="danger"
        title={archiving ? t("account.archiveKeyConfirm", "Archive {name}?", { name: archiving.name }) : t("account.archiveKeyConfirmAny", "Archive this key?")}
        description={t("account.archiveKeyConfirmHint", "Applications using it stop authenticating; archiving can't be undone, and its history stays.")}
        confirmLabel={t("account.archiveKey", "Archive key")}
        loading={act.isPending}
        onConfirm={() => (archiving ? act.mutateAsync({ key: archiving, action: "archive" }) : undefined)}
      />
    </SettingsPage>
  );
}
