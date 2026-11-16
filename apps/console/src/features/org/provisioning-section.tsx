import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertDialog, Button, Code, CopyButton, Dialog, DialogScope, SettingsGroup, SettingsRow, Section } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { ScimStatus } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { toast } from "~/shell/toast";

const scimQuery = (org: string) => ({
  queryKey: ["orgs", org, "scim"] as const,
  queryFn: async (): Promise<ScimStatus> => data(await api.GET("/platform/v1/orgs/{org}/scim", { params: { path: { org } } })),
});

/** Security › Provisioning: the SCIM base URL and token an identity provider uses to add and remove members. */
export function Provisioning({ org, connected }: { org: string; connected: boolean }) {
  const needs = useNeedsRight(org);
  const canManage = useOrgRights(org).has("security.sso.manage");
  const status = useQuery(scimQuery(org));
  const [token, setToken] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const rotate = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/scim/token", { params: { path: { org } } })),
    invalidates: [["orgs", org, "scim"]],
    onSuccess: (created) => setToken(created.token),
  });
  const remove = useApiMutation({
    mutationFn: async () => data(await api.DELETE("/platform/v1/orgs/{org}/scim/token", { params: { path: { org } } })),
    invalidates: [["orgs", org, "scim"]],
    onSuccess: () => {
      setDeleting(false);
      toast.success(t("org.scim.deleted", "SCIM token deleted"));
    },
  });
  const s = status.data;
  const reason = !canManage ? needs("security.sso.manage") : !connected ? t("org.sso.connectFirst", "Connect an identity provider first") : undefined;
  return (
    <Section
      spacing="settings"
      title={t("org.scim.title", "Provisioning")}
      description={t("org.scim.hint", "Your identity provider adds and removes members through SCIM.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("org.scim.baseUrl", "SCIM base URL")}
          control={s ? <CopyButton display="inline" value={s.base_url} label={t("org.scim.copyBaseUrl", "Copy SCIM base URL")} /> : undefined}
          description={s?.base_url}
        />
        <SettingsRow
          title={t("org.scim.token", "Token")}
          description={
            s?.enabled
              ? t("org.scim.tokenEnding", "Ends in {last4}", { last4: s.last4 ?? "" })
              : t("org.scim.noToken", "No token: the identity provider can't provision.")
          }
          control={
            <div className="flex gap-8">
              {s?.enabled && (
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={!canManage}
                  {...(canManage ? {} : { disabledReason: needs("security.sso.manage") })}
                  onClick={() => setDeleting(true)}
                >
                  {common.delete()}
                </Button>
              )}
              <Button
                size="sm"
                disabled={!s || !!reason}
                {...(reason ? { disabledReason: reason } : {})}
                loading={rotate.isPending}
                onClick={() => rotate.mutate()}
              >
                {s?.enabled ? t("org.scim.replaceToken", "Replace token") : t("org.scim.createToken", "Create token")}
              </Button>
            </div>
          }
        />
        {s?.enabled && (
          <SettingsRow
            title={t("org.scim.activity", "Activity")}
            description={
              s.last_used_at
                ? t("org.scim.lastUsed", "Last request {when}; {count} members provisioned", { when: formatRelative(s.last_used_at), count: s.provisioned })
                : t("org.scim.neverUsed", "No request yet")
            }
          />
        )}
        {s?.last_error && (
          <SettingsRow
            title={t("org.scim.lastError", "Last error")}
            description={s.last_error_at ? formatRelative(s.last_error_at) : undefined}
            control={<Code>{s.last_error}</Code>}
          />
        )}
      </SettingsGroup>
      <DialogScope open={!!token}>
        <Dialog
          open={!!token}
          onOpenChange={(o) => !o && setToken(null)}
          size="sm"
          title={t("org.scim.created", "SCIM token created")}
          description={t("org.scim.createdHint", "Copy it into your identity provider now; it's shown once, and a replaced token works 24 more hours.")}
          footer={<Button onClick={() => setToken(null)}>{common.finish()}</Button>}
        >
          <div className="flex items-center gap-8">
            <Code>{token}</Code>
            {token && <CopyButton value={token} label={t("org.scim.copyToken", "Copy token")} />}
          </div>
        </Dialog>
      </DialogScope>
      <AlertDialog
        open={deleting}
        onOpenChange={setDeleting}
        tone="danger"
        title={t("org.scim.deleteConfirm", "Delete the SCIM token?")}
        description={t("org.scim.deleteConfirmHint", "The identity provider stops adding and removing members at once. Members stay.")}
        confirmLabel={common.delete()}
        loading={remove.isPending}
        onConfirm={() => remove.mutateAsync()}
      />
    </Section>
  );
}
