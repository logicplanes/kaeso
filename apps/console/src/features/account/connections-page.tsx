import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AlertDialog, Button, Dialog, DialogScope, Empty, Section, SettingsGroup, SettingsRow, Status } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { PersonConnectionView } from "~/api/generated/types";
import { myConnectionsQuery } from "~/api/queries/connections";
import { useApiMutation } from "~/auth/mutation";
import { useSession } from "~/auth/session";
import { LoadFailed } from "~/features/org/load-failed";
import { signInToast, type SignInResult } from "~/features/org-home/mcp-servers-page";
import { t } from "~/i18n";
import { HeaderFields } from "~/kaeso/mcp-header-fields";
import type { Word } from "~/kaeso/mcp-servers";
import { SettingsLoading, SettingsPage } from "~/shell/settings-page";
import { toast } from "~/shell/toast";

export interface ConnectionsSearch {
  server?: string;
  result?: SignInResult;
}

const KEY = myConnectionsQuery.queryKey;

function wordOf(c: PersonConnectionView): Word {
  const own = c.connection;
  if (!own) return { tone: "neutral", label: t("account.connections.notConnected", "Not connected") };
  if (own.signed_out) return { tone: "warning", label: t("orgHome.mcp.connection.signedOut", "Signed out") };
  if (own.account) return { tone: "success", label: t("orgHome.mcp.connection.account", "Connected as {account}", { account: own.account }) };
  return { tone: "success", label: t("orgHome.mcp.connection.connected", "Connected") };
}

export function ConnectionsPage({ search = {} }: { search?: ConnectionsSearch }) {
  const navigate = useNavigate();
  const session = useSession();
  const connections = useQuery(myConnectionsQuery);
  const [header, setHeader] = useState<PersonConnectionView | null>(null);
  const [leaving, setLeaving] = useState<PersonConnectionView | null>(null);
  const [leaveOpen, setLeaveOpen] = useState(false);
  const list = useMemo(() => connections.data ?? [], [connections.data]);
  const orgName = (org: string) => session.memberships.find((m) => m.org_id === org)?.org_name ?? org;
  const result = search.result;
  const loaded = !connections.isPending;
  useEffect(() => {
    if (!result || !loaded) return;
    signInToast(result, list.find((c) => c.server === search.server)?.server_name);
    void navigate({ to: "/settings/account/connections", search: {}, replace: true });
  }, [result, loaded, list, search.server, navigate]);
  const signIn = useApiMutation({
    mutationFn: async (c: PersonConnectionView) =>
      data(await api.POST("/platform/v1/me/mcp-connections/{server}", { params: { path: { server: c.server } }, body: { oauth: true } })),
    onSuccess: (answer) => {
      if (answer.redirect_url) window.location.assign(answer.redirect_url);
    },
  });
  const connect = (c: PersonConnectionView) => (c.auth_method === "header" ? setHeader(c) : signIn.mutate(c));
  return (
    <SettingsPage
      title={t("account.connections.title", "Connections")}
      context={t("account.title", "Account")}
      description={t("account.connections.hint", "MCP servers where you sign in with your own account.")}
    >
      {connections.isError ? (
        <LoadFailed title={t("account.connections.failed", "Your connections couldn't load")} />
      ) : connections.isPending ? (
        <SettingsLoading rows={2} />
      ) : list.length === 0 ? (
        <Empty
          title={t("account.connections.empty", "No servers ask for your own account")}
          description={t("account.connections.emptyHint", "When an organization adds a server where each person signs in, it shows here.")}
        />
      ) : (
        <Section spacing="settings">
          <SettingsGroup>
            {list.map((c) => {
              const word = wordOf(c);
              const own = c.connection;
              return (
                <SettingsRow
                  key={c.server}
                  title={
                    <span className="flex flex-wrap items-center gap-8">
                      {c.server_name}
                      <Status tone={word.tone}>{word.label}</Status>
                    </span>
                  }
                  description={orgName(c.org)}
                  control={
                    <span className="flex gap-8">
                      {(!own || own.signed_out || c.auth_method === "header") && (
                        <Button size="sm" loading={signIn.isPending && signIn.variables?.server === c.server} onClick={() => connect(c)}>
                          {!own
                            ? t("account.connections.connect", "Connect")
                            : c.auth_method === "header"
                              ? t("orgHome.mcp.replaceSecret", "Replace secret")
                              : t("orgHome.mcp.signInAgain", "Sign in again")}
                        </Button>
                      )}
                      {own && (
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => {
                            setLeaving(c);
                            setLeaveOpen(true);
                          }}
                        >
                          {t("account.connections.disconnect", "Disconnect")}
                        </Button>
                      )}
                    </span>
                  }
                />
              );
            })}
          </SettingsGroup>
        </Section>
      )}
      <DialogScope open={!!header}>{header && <HeaderDialog connection={header} onClose={() => setHeader(null)} />}</DialogScope>
      {leaving && (
        <DialogScope open={leaveOpen}>
          <DisconnectDialog connection={leaving} open={leaveOpen} onOpenChange={setLeaveOpen} />
        </DialogScope>
      )}
    </SettingsPage>
  );
}

function HeaderDialog({ connection, onClose }: { connection: PersonConnectionView; onClose: () => void }) {
  const [name, setName] = useState(connection.connection?.header_name ?? "Authorization");
  const [value, setValue] = useState("");
  const save = useApiMutation({
    mutationFn: async () =>
      data(
        await api.POST("/platform/v1/me/mcp-connections/{server}", {
          params: { path: { server: connection.server } },
          body: { header: { name: name.trim(), value: value.trim() } },
        }),
      ),
    invalidates: [KEY],
    onSuccess: () => {
      setValue("");
      toast.success(t("account.connections.connected", "Connected to {name}", { name: connection.server_name }));
      onClose();
    },
  });
  const blocked = !name.trim()
    ? t("orgHome.mcp.headerNameNeeded", "Name the header")
    : !value.trim()
      ? t("orgHome.mcp.headerValueNeeded", "Paste the header's value")
      : undefined;
  return (
    <Dialog
      busy={save.isPending}
      open
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={t("account.connections.headerTitle", "Connect to {name}", { name: connection.server_name })}
      description={t("account.connections.headerHint", "Kaeso sends this header with your calls to the server and never shows it again.")}
      onSubmit={() => !blocked && save.mutate()}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button
            variant="primary"
            disabled={!!blocked}
            {...(blocked ? { disabledReason: blocked } : {})}
            loading={save.isPending}
            onClick={() => save.mutate()}
          >
            {t("account.connections.connect", "Connect")}
          </Button>
        </>
      }
    >
      <HeaderFields name={name} value={value} onName={setName} onValue={setValue} />
    </Dialog>
  );
}

function DisconnectDialog({ connection, open, onOpenChange }: { connection: PersonConnectionView; open: boolean; onOpenChange: (open: boolean) => void }) {
  const remove = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/me/mcp-connections/{server}", { params: { path: { server: connection.server } } });
    },
    invalidates: [KEY],
    onSuccess: () => {
      onOpenChange(false);
      toast(t("account.connections.disconnected", "Disconnected from {name}", { name: connection.server_name }));
    },
    onError: () => onOpenChange(false),
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("account.connections.disconnectTitle", "Disconnect from {name}?", { name: connection.server_name })}
      description={t("account.connections.disconnectHint", "Its tools stop working for you until you connect again.")}
      confirmLabel={t("account.connections.disconnect", "Disconnect")}
      loading={remove.isPending}
      onConfirm={() => remove.mutateAsync()}
    />
  );
}
