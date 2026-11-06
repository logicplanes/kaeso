import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertDialog,
  Button,
  ClientButton,
  Code,
  CodeBlock,
  CopyButton,
  Dialog,
  DialogScope,
  Empty,
  List,
  Section,
  Text,
  type ClientMark,
} from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { api } from "~/api/client";
import type { PortalGrantView } from "~/api/generated/types";
import { projectToolViews } from "~/api/queries/invalidation";
import { portalGrantsQuery } from "~/api/queries/tools";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useProjectRights } from "~/auth/rights";
import { usePartyName } from "~/features/project/keys/party";
import { t } from "~/i18n";
import { LastUsed } from "~/kaeso/last-used";
import { hostOf, portalUrl } from "~/kaeso/mcp-servers";
import { toast } from "~/shell/toast";

const VSCODE_CONFIG = ".vscode/mcp.json";
const CLIENTS: readonly ClientMark[] = ["claude", "chatgpt", "cursor", "github-copilot"];

const clientName = (client: ClientMark): string =>
  client === "claude" ? "Claude" : client === "chatgpt" ? "ChatGPT" : client === "cursor" ? "Cursor" : "GitHub Copilot";

// Install links as Cursor and VS Code document them: https://cursor.com/docs/context/mcp/install-links and
// https://code.visualstudio.com/docs/copilot/guides/mcp-developer-guide
const cursorLink = (url: string): string => `cursor://anysphere.cursor-deeplink/mcp/install?name=kaeso&config=${btoa(JSON.stringify({ url }))}`;
const vscodeLink = (url: string): string => `vscode:mcp/install?${encodeURIComponent(JSON.stringify({ name: "kaeso", type: "http", url }))}`;

function Steps({ client, url }: { client: ClientMark; url: string }) {
  if (client === "claude")
    return (
      <div className="flex flex-col gap-12">
        <Text>{t("project.tools.claudeSteps", "In Claude, open Settings › Connectors, choose Add custom connector and paste the portal address.")}</Text>
        <CodeBlock copy title={t("project.tools.claudeCode", "Claude Code")} lang="bash" code={`claude mcp add --transport http kaeso ${url}`} />
      </div>
    );
  if (client === "chatgpt")
    return <Text>{t("project.tools.chatgptSteps", "In ChatGPT, open Settings › Apps & Connectors, create a connector and paste the portal address.")}</Text>;
  if (client === "cursor")
    return (
      <Button variant="primary" fullWidth href={cursorLink(url)} target="_self">
        {t("account.addToCursor", "Add to Cursor")}
      </Button>
    );
  return (
    <div className="flex flex-col gap-12">
      <Button variant="primary" fullWidth href={vscodeLink(url)} target="_self">
        {t("account.addToVsCode", "Add to VS Code")}
      </Button>
      <CodeBlock copy title={VSCODE_CONFIG} lang="json" code={JSON.stringify({ servers: { kaeso: { type: "http", url } } }, null, 2)} />
    </div>
  );
}

export function ConnectTab({ org, project }: { org: string; project: string }) {
  const url = portalUrl(project);
  const [client, setClient] = useState<ClientMark | null>(null);
  const [shown, setShown] = useState<ClientMark>("claude");
  return (
    <div className="flex flex-col gap-32">
      <Section
        spacing="none"
        title={t("project.tools.address", "Portal address")}
        description={t("project.tools.addressHint", "Clients add this address and sign in with a Kaeso account.")}
      >
        <span className="flex min-w-0 items-center gap-8">
          <Code>{url}</Code>
          <CopyButton value={url} label={t("project.tools.copyAddress", "Copy portal address")} />
        </span>
      </Section>
      <Section
        spacing="none"
        title={t("project.tools.addToClient", "Add to a client")}
        description={t("project.tools.addToClientHint", "Each person signs in and allows the client in their browser.")}
      >
        <div className="flex flex-col gap-16">
          <span className="flex gap-8">
            {CLIENTS.map((c) => (
              <ClientButton
                key={c}
                client={c}
                aria-label={t("account.connectClient", "Add to {client}", { client: clientName(c) })}
                onClick={() => {
                  setShown(c);
                  setClient(c);
                }}
              />
            ))}
          </span>
          <Text variant="small" tone="secondary">
            {t("project.tools.ownAgent", "Your own agent calls the portal with an API key that may use tools.")}
          </Text>
          <span>
            <Button size="sm" href={`/orgs/${org}/projects/${project}/keys?create=true`}>
              {t("project.tools.createKey", "Create an API key")}
            </Button>
          </span>
        </div>
      </Section>
      <ConnectedClients org={org} project={project} />
      <Dialog
        open={client !== null}
        onOpenChange={(open) => !open && setClient(null)}
        title={t("project.tools.connectTitle", "Add this portal to {client}", { client: clientName(shown) })}
        description={t("project.tools.connectLead", "The client asks you to sign in to Kaeso and to allow it.")}
        footer={
          <Dialog.Close>
            <Button variant="primary">{t("common.finish", "Finish")}</Button>
          </Dialog.Close>
        }
      >
        <Steps client={shown} url={url} />
      </Dialog>
    </div>
  );
}

function ConnectedClients({ org, project }: { org: string; project: string }) {
  const rights = useProjectRights(org, project);
  const needs = useNeedsRight(org, project);
  const partyName = usePartyName(org, project);
  const canManage = rights.has("tools.manage");
  const grants = useQuery(portalGrantsQuery(org, project));
  const [leaving, setLeaving] = useState<PortalGrantView | null>(null);
  const [open, setOpen] = useState(false);
  const list = grants.data ?? [];
  return (
    <Section title={t("project.tools.connected", "Connected clients")} spacing="none">
      <List
        aria-label={t("project.tools.connected", "Connected clients")}
        loading={grants.isPending}
        empty={
          <Empty
            size="compact"
            title={t("project.tools.noClients", "No clients connected yet")}
            description={t("project.tools.noClientsHint", "Clients appear here once someone signs in to the portal from them.")}
          />
        }
        items={list.map((g) => ({
          id: g.id,
          textValue: g.client_name,
          title: g.client_name,
          subtitle: t("project.tools.clientFor", "{person} · receives sign-ins at {host}", {
            person: partyName({ kind: "user", id: g.user }),
            host: hostOf(g.redirect_uri),
          }),
          meta: (
            <Text variant="small" tone="secondary">
              {t("project.tools.connectedAt", "Connected {when}", { when: formatRelative(g.created_at) })}
            </Text>
          ),
          date: <LastUsed at={g.last_used_at} />,
          actions: (
            <Button
              size="xs"
              disabled={!canManage}
              {...(canManage ? {} : { disabledReason: needs("tools.manage") })}
              onClick={() => {
                setLeaving(g);
                setOpen(true);
              }}
            >
              {t("project.tools.disconnect", "Disconnect")}
            </Button>
          ),
        }))}
      />
      {leaving && (
        <DialogScope open={open}>
          <Disconnect org={org} project={project} grant={leaving} open={open} onOpenChange={setOpen} />
        </DialogScope>
      )}
    </Section>
  );
}

function Disconnect({
  org,
  project,
  grant,
  open,
  onOpenChange,
}: {
  org: string;
  project: string;
  grant: PortalGrantView;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const remove = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/orgs/{org}/projects/{project}/portal-grants/{grant}", { params: { path: { org, project, grant: grant.id } } });
    },
    invalidates: [projectToolViews(org)],
    onSuccess: () => {
      onOpenChange(false);
      toast(t("project.tools.disconnected", "{client} disconnected", { client: grant.client_name }));
    },
    onError: () => onOpenChange(false),
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("project.tools.disconnectTitle", "Disconnect {client}?", { client: grant.client_name })}
      description={t("project.tools.disconnectHint", "Its tokens stop working now; it has to sign in again to call tools.")}
      confirmLabel={t("project.tools.disconnect", "Disconnect")}
      loading={remove.isPending}
      onConfirm={() => remove.mutateAsync()}
    />
  );
}
