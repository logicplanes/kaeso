import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertDialog,
  Banner,
  Button,
  Checkbox,
  CheckboxGroup,
  Code,
  DiffView,
  Dialog,
  DialogScope,
  Empty,
  Input,
  KeyValue,
  Link,
  List,
  Menu,
  Section,
  Sheet,
  Skeleton,
  Status,
  Switch,
  Text,
  icons,
} from "@logicplanes/kaeso-ui";
import { formatDate, formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data, ifMatch } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { McpServerView, McpToolView, ProjectView } from "~/api/generated/types";
import { mcpServerQuery, mcpServerToolsQuery, mcpToolChangeQuery } from "~/api/queries/mcp-servers";
import { projectToolViews } from "~/api/queries/invalidation";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { useOrgPartyName } from "~/features/org/party";
import { t } from "~/i18n";
import { HANDLE, MAX_HANDLE, toolPrefix, connectionWord, hostOf, serverInfoText, statusWord, toolStateWord } from "~/kaeso/mcp-servers";
import { HeaderFields } from "~/kaeso/mcp-header-fields";
import { useMcpSignIn } from "~/kaeso/mcp-sign-in";
import { toast } from "~/shell/toast";

const SERVERS = (org: string) => ["orgs", org, "mcp-servers"];

export function McpServerSheet({ org, serverId, onClose }: { org: string; serverId: string | undefined; onClose: () => void }) {
  const server = useQuery({ ...mcpServerQuery(org, serverId ?? ""), enabled: !!serverId });
  return (
    <Sheet
      open={!!serverId}
      onOpenChange={(o) => !o && onClose()}
      modal={false}
      size="md"
      title={server.data?.name ?? t("orgHome.mcp.server", "Server")}
      {...(server.data ? { subtitle: hostOf(server.data.url) } : {})}
    >
      {serverId && <ServerBody key={serverId} org={org} serverId={serverId} onClose={onClose} />}
    </Sheet>
  );
}

function ServerBody({ org, serverId, onClose }: { org: string; serverId: string; onClose: () => void }) {
  const rights = useOrgRights(org);
  const needs = useNeedsRight(org);
  const canManage = rights.has("mcp_servers.manage");
  const reason = canManage ? undefined : needs("mcp_servers.manage");
  const server = useQuery(mcpServerQuery(org, serverId));
  const tools = useQuery(mcpServerToolsQuery(org, serverId));
  const projects = useQuery(orgProjectsQuery(org));
  const partyName = useOrgPartyName(org);
  const [editing, setEditing] = useState(false);
  const [assigning, setAssigning] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [replacing, setReplacing] = useState(false);
  const [reviewing, setReviewing] = useState<McpToolView | null>(null);
  const signIn = useMcpSignIn(org);
  const all = projects.data ?? [];
  const refresh = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/mcp-servers/{server}/refresh", { params: { path: { org, server: serverId } } })),
    invalidates: [SERVERS(org)],
    onSuccess: (found) =>
      toast(
        found.added.length + found.held.length === 0
          ? t("orgHome.mcp.refreshedNothing", "No new or changed tools")
          : t("orgHome.mcp.refreshed", "{added} new and {held} changed tools found", { added: found.added.length, held: found.held.length }),
      ),
  });

  if (server.isPending) return <Skeleton lines={4} />;
  if (server.isError)
    return (
      <Empty
        title={
          isApiError(server.error) && server.error.type === "not_found"
            ? t("orgHome.mcp.notFound", "This server doesn't exist or was deleted.")
            : t("orgHome.mcp.oneLoadFailed", "Couldn't load this server.")
        }
        actions={<Button onClick={onClose}>{t("orgHome.mcp.backToServers", "Back to servers")}</Button>}
      />
    );
  const s = server.data;
  const status = statusWord(s.status);
  const connection = connectionWord(s);
  const held = (tools.data ?? []).filter((x) => x.state === "changed");
  const info = serverInfoText(s.server_info);
  const names = s.projects.map((id) => all.find((p) => p.id === id)?.name ?? id);
  const shared = s.auth_method !== "none" && s.credential_mode === "shared";
  const manage = (label: string, onClick: () => void, extra: { loading?: boolean } = {}) => (
    <Button size="sm" disabled={!canManage} {...(reason ? { disabledReason: reason } : {})} onClick={onClick} {...extra}>
      {label}
    </Button>
  );

  return (
    <div className="flex flex-col gap-24">
      <div className="flex items-center justify-between gap-12">
        <Status tone={status.tone}>{status.label}</Status>
        <Menu>
          <Menu.Trigger>
            <Button
              variant="ghost"
              size="sm"
              icon={icons.more}
              aria-label={t("orgHome.mcp.actions", "Server actions")}
              disabled={!canManage}
              {...(reason ? { disabledReason: reason } : {})}
            />
          </Menu.Trigger>
          <Menu.Content align="end">
            <Menu.Item icon={icons.refresh} onClick={() => refresh.mutate()}>
              {t("orgHome.mcp.refresh", "Check for new tools")}
            </Menu.Item>
            <Menu.Item icon={icons.edit} onClick={() => setEditing(true)}>
              {t("orgHome.mcp.edit", "Edit")}
            </Menu.Item>
            <Menu.Item icon={icons.delete} variant="danger" onClick={() => setDeleting(true)}>
              {t("common.delete", "Delete")}
            </Menu.Item>
          </Menu.Content>
        </Menu>
      </div>
      {held.length > 0 && (
        <Banner
          tone="warning"
          title={t("orgHome.mcp.heldTitle", "{count, plural, one {# tool waits} other {# tools wait}} for review", { count: held.length })}
        >
          {t("orgHome.mcp.heldHint", "Projects stop offering a changed tool until someone approves the change.")}
        </Banner>
      )}
      <KeyValue
        items={[
          { label: t("orgHome.mcp.address", "Address"), value: s.url, copy: s.url, wrap: true },
          { label: t("orgHome.mcp.handle", "Handle"), value: <Code>{s.handle}</Code> },
          ...(info ? [{ label: t("orgHome.mcp.serverInfo", "Says it is"), value: info }] : []),
          ...(s.protocol_version ? [{ label: t("orgHome.mcp.protocol", "MCP version"), value: s.protocol_version, mono: true }] : []),
          {
            label: t("orgHome.mcp.changes", "Changed tools"),
            value: s.change_policy === "hold" ? t("orgHome.mcp.policy.hold", "Held for review") : t("orgHome.mcp.policy.accept", "Offered as they come"),
          },
          { label: t("orgHome.mcp.checked", "Checked"), value: s.last_checked_at ? formatRelative(s.last_checked_at) : t("common.never", "Never") },
          ...(s.last_error ? [{ label: t("orgHome.mcp.lastError", "Last error"), value: s.last_error, wrap: true }] : []),
          {
            label: t("orgHome.added", "Added"),
            value: t("orgHome.addedBy", "{name}, {date}", { name: partyName(s.created_by), date: formatDate(s.created_at) }),
          },
          { label: t("orgHome.mcp.id", "Id"), value: s.id, copy: s.id },
        ]}
      />
      <Section
        title={t("orgHome.mcp.connection", "Connection")}
        spacing="none"
        actions={
          shared &&
          (s.auth_method === "oauth"
            ? manage(t("orgHome.mcp.signInAgain", "Sign in again"), () => signIn.start(async () => s.id), { loading: signIn.starting })
            : manage(t("orgHome.mcp.replaceSecret", "Replace secret"), () => setReplacing(true)))
        }
      >
        <div className="flex flex-col gap-12">
          <Status tone={connection.tone}>{connection.label}</Status>
          {s.credential_mode === "per_user" && (
            <Text variant="small" tone="secondary">
              {t("orgHome.mcp.perUserHint", "Each person connects their own account under Account › Connections.")}
            </Text>
          )}
          {shared && s.connection && (
            <KeyValue
              items={[
                ...(s.connection.header_name
                  ? [{ label: t("orgHome.mcp.header", "Header"), value: `${s.connection.header_name}: …${s.connection.last4}`, mono: true }]
                  : []),
                { label: t("orgHome.mcp.signedIn", "Signed in"), value: formatRelative(s.connection.updated_at) },
                ...(s.connection.expires_at ? [{ label: t("orgHome.mcp.renews", "Renews"), value: formatRelative(s.connection.expires_at) }] : []),
                {
                  label: t("common.lastUsed", "Last used"),
                  value: s.connection.last_used_at ? formatRelative(s.connection.last_used_at) : t("common.never", "Never"),
                },
              ]}
            />
          )}
          {signIn.waiting && (
            <Text variant="small" tone="secondary">
              {t("orgHome.mcp.finishInTab", "Finish the sign-in in the new tab.")}
            </Text>
          )}
          {signIn.link && (
            <Link href={signIn.link} external>
              {t("orgHome.mcp.openSignIn", "Open the sign-in page")}
            </Link>
          )}
        </div>
      </Section>
      <Section
        title={t("orgHome.mcp.tools", "Tools")}
        spacing="none"
        actions={manage(t("orgHome.mcp.refresh", "Check for new tools"), () => refresh.mutate(), { loading: refresh.isPending })}
      >
        <List
          aria-label={t("orgHome.mcp.tools", "Tools")}
          loading={tools.isPending}
          empty={
            <Empty
              size="compact"
              title={t("orgHome.mcp.noTools", "No tools listed yet")}
              description={
                s.credential_mode === "per_user"
                  ? t("orgHome.mcp.noToolsPerUser", "Tools appear once someone connects an account.")
                  : t("orgHome.mcp.noToolsHint", "Tools appear once Kaeso can sign in and list them.")
              }
            />
          }
          onOpen={(item) => {
            const tool = tools.data?.find((x) => x.id === item.id);
            if (tool?.state === "changed") setReviewing(tool);
          }}
          items={(tools.data ?? []).map((tool) => {
            const state = toolStateWord(tool.state);
            return {
              id: tool.id,
              title: <Code>{tool.exposed_name}</Code>,
              textValue: tool.exposed_name,
              subtitle: tool.title || tool.description || undefined,
              meta: <Status tone={state.tone}>{state.label}</Status>,
              ...(tool.state === "changed"
                ? {
                    actions: (
                      <Button size="xs" onClick={() => setReviewing(tool)}>
                        {t("orgHome.mcp.review", "Review")}
                      </Button>
                    ),
                  }
                : {}),
            };
          })}
        />
      </Section>
      <Section
        title={t("common.projects", "Projects")}
        spacing="none"
        actions={manage(t("orgHome.mcp.chooseProjects", "Choose projects"), () => setAssigning(true))}
      >
        <Text variant="small" tone={names.length ? "secondary" : "tertiary"}>
          {names.length ? names.join(t("orgHome.listSeparator", ", ")) : t("orgHome.mcp.noProjects", "No project uses this server.")}
        </Text>
      </Section>
      <DialogScope open={editing}>
        <EditServerDialog org={org} server={s} open={editing} onClose={() => setEditing(false)} />
      </DialogScope>
      <DialogScope open={assigning}>
        <ServerProjectsDialog org={org} server={s} projects={all} open={assigning} onClose={() => setAssigning(false)} />
      </DialogScope>
      <DialogScope open={replacing}>
        <ReplaceSecretDialog org={org} server={s} open={replacing} onClose={() => setReplacing(false)} />
      </DialogScope>
      <DialogScope open={deleting}>
        <DeleteServerDialog org={org} server={s} open={deleting} onOpenChange={setDeleting} onDeleted={onClose} />
      </DialogScope>
      <ToolReviewSheet org={org} server={s} tool={reviewing} canManage={canManage} onClose={() => setReviewing(null)} />
    </div>
  );
}

const pretty = (v: unknown) => (v === undefined || v === null ? "" : JSON.stringify(v, null, 2));

type Definition = { title?: unknown; description?: unknown; inputSchema?: unknown; outputSchema?: unknown; annotations?: unknown };

function ToolReviewSheet({
  org,
  server,
  tool,
  canManage,
  onClose,
}: {
  org: string;
  server: McpServerView;
  tool: McpToolView | null;
  canManage: boolean;
  onClose: () => void;
}) {
  return (
    <Sheet
      open={!!tool}
      onOpenChange={(o) => !o && onClose()}
      size="lg"
      title={tool ? t("orgHome.mcp.reviewTitle", "Review {tool}", { tool: tool.exposed_name }) : ""}
      subtitle={server.name}
      {...(tool ? { footer: <ReviewActions org={org} server={server} tool={tool} canManage={canManage} onDone={onClose} /> } : {})}
    >
      {tool && <ReviewBody key={tool.id} org={org} server={server} tool={tool} />}
    </Sheet>
  );
}

function ReviewBody({ org, server, tool }: { org: string; server: McpServerView; tool: McpToolView }) {
  const change = useQuery(mcpToolChangeQuery(org, server.id, tool.id));
  if (change.isPending) return <Skeleton lines={6} />;
  if (change.isError) return <Empty title={t("orgHome.mcp.changeFailed", "This change couldn't load")} description={change.error.message} />;
  const before = (change.data.approved ?? {}) as Definition;
  const after = change.data.current as Definition;
  const parts: { id: string; title: string; before: string; after: string; json: boolean }[] = [
    { id: "title", title: t("orgHome.mcp.part.title", "Title"), before: String(before.title ?? ""), after: String(after.title ?? ""), json: false },
    {
      id: "description",
      title: t("orgHome.mcp.part.description", "Description"),
      before: String(before.description ?? ""),
      after: String(after.description ?? ""),
      json: false,
    },
    { id: "input", title: t("orgHome.mcp.part.input", "Input schema"), before: pretty(before.inputSchema), after: pretty(after.inputSchema), json: true },
    { id: "output", title: t("orgHome.mcp.part.output", "Output schema"), before: pretty(before.outputSchema), after: pretty(after.outputSchema), json: true },
    { id: "hints", title: t("orgHome.mcp.part.hints", "Hints"), before: pretty(before.annotations), after: pretty(after.annotations), json: true },
  ].filter((p) => p.before !== p.after);
  return (
    <div className="flex flex-col gap-24">
      <Banner tone="warning" title={t("orgHome.mcp.heldBecause", "{server} changed this tool", { server: server.name })}>
        {t("orgHome.mcp.heldBecauseHint", "Projects stop offering it until someone approves the change.")}
      </Banner>
      {tool.long_description && (
        <Banner tone="info">{t("orgHome.mcp.longDescription", "Its description is longer than 2,000 characters; read all of it before you approve.")}</Banner>
      )}
      {parts.map((p) => (
        <DiffView key={p.id} title={p.title} before={p.before} after={p.after} wrap {...(p.json ? { lang: "json" as const } : {})} />
      ))}
      <KeyValue
        items={[
          { label: t("orgHome.mcp.approvedHash", "Approved definition"), value: tool.approved_hash ?? "—", mono: true, wrap: true },
          { label: t("orgHome.mcp.currentHash", "Current definition"), value: tool.hash, mono: true, wrap: true },
        ]}
      />
    </div>
  );
}

function ReviewActions({
  org,
  server,
  tool,
  canManage,
  onDone,
}: {
  org: string;
  server: McpServerView;
  tool: McpToolView;
  canManage: boolean;
  onDone: () => void;
}) {
  const needs = useNeedsRight(org);
  const reason = canManage ? undefined : needs("mcp_servers.manage");
  const path = { params: { path: { org, server: server.id, tool: tool.id } } };
  const approve = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/mcp-servers/{server}/tools/{tool}/approve", { ...path, body: { hash: tool.hash } })),
    invalidates: [SERVERS(org)],
    onSuccess: (x) => {
      toast.success(t("orgHome.mcp.approved", "{tool} approved", { tool: x.exposed_name }));
      onDone();
    },
  });
  const reject = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/mcp-servers/{server}/tools/{tool}/reject", path)),
    invalidates: [SERVERS(org)],
    onSuccess: (x) => {
      toast(t("orgHome.mcp.keptOff", "{tool} stays off", { tool: x.exposed_name }));
      onDone();
    },
  });
  const blocked = reason ? { disabled: true, disabledReason: reason } : {};
  return (
    <>
      <Button loading={reject.isPending} onClick={() => reject.mutate()} {...blocked}>
        {t("orgHome.mcp.keepOff", "Keep off")}
      </Button>
      <Button variant="primary" loading={approve.isPending} onClick={() => approve.mutate()} {...blocked}>
        {t("orgHome.mcp.approve", "Approve change")}
      </Button>
    </>
  );
}

function EditServerDialog({ org, server, open, onClose }: { org: string; server: McpServerView; open: boolean; onClose: () => void }) {
  const [name, setName] = useState(server.name);
  const [handle, setHandle] = useState(server.handle);
  const [hold, setHold] = useState(server.change_policy === "hold");
  const save = useApiMutation({
    mutationFn: async () =>
      data(
        await api.PATCH("/platform/v1/orgs/{org}/mcp-servers/{server}", {
          params: { path: { org, server: server.id }, header: ifMatch(server) },
          body: {
            ...(name.trim() !== server.name ? { name: name.trim() } : {}),
            ...(handle !== server.handle ? { handle } : {}),
            change_policy: hold ? "hold" : "accept",
          },
        }),
      ),
    invalidates: [SERVERS(org)],
    onSuccess: (s) => {
      toast(t("orgHome.mcp.saved", "{name} saved", { name: s.name }));
      onClose();
    },
  });
  const blocked = !name.trim()
    ? t("orgHome.mcp.nameNeeded", "Give the server a name")
    : !HANDLE.test(handle) || handle.length > MAX_HANDLE
      ? t("orgHome.mcp.handleNeeded", "Use 1 to 24 lowercase letters, digits and hyphens")
      : undefined;
  return (
    <Dialog
      busy={save.isPending}
      open={open}
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={t("orgHome.mcp.editTitle", "Edit {name}", { name: server.name })}
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
            {t("common.save", "Save")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-16">
        <Input label={t("common.name", "Name")} value={name} onChange={(e) => setName(e.target.value)} maxLength={100} autoFocus />
        <Input
          label={t("orgHome.mcp.handle", "Handle")}
          description={t("orgHome.mcp.handleHint", "Its tools are named {prefix}tool outside Kaeso.", { prefix: toolPrefix(handle) })}
          value={handle}
          onChange={(e) => setHandle(e.target.value.toLowerCase())}
          maxLength={MAX_HANDLE}
          spellCheck={false}
        />
        <Switch
          label={t("orgHome.mcp.holdChanges", "Hold changed tools for review")}
          description={t("orgHome.mcp.holdChangesHint", "A changed tool stays off until someone approves it.")}
          checked={hold}
          onCheckedChange={setHold}
        />
      </div>
    </Dialog>
  );
}

function ServerProjectsDialog({
  org,
  server,
  projects,
  open,
  onClose,
}: {
  org: string;
  server: McpServerView;
  projects: ProjectView[];
  open: boolean;
  onClose: () => void;
}) {
  const [chosen, setChosen] = useState<string[]>(server.projects);
  const save = useApiMutation({
    mutationFn: async () =>
      data(
        await api.PATCH("/platform/v1/orgs/{org}/mcp-servers/{server}", {
          params: { path: { org, server: server.id }, header: ifMatch(server) },
          body: { projects: chosen },
        }),
      ),
    invalidates: [SERVERS(org), projectToolViews(org)],
    onSuccess: (s) => {
      toast(
        s.projects.length === 0
          ? t("orgHome.mcp.unassigned", "No project uses {name} now", { name: s.name })
          : t("orgHome.mcp.assigned", "{name} is in {count, plural, one {# project} other {# projects}}", { name: s.name, count: s.projects.length }),
      );
      onClose();
    },
  });
  return (
    <Dialog
      busy={save.isPending}
      open={open}
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={t("orgHome.mcp.projectsTitle", "Projects that use {name}", { name: server.name })}
      description={t("orgHome.mcp.projectsHint", "Each project still chooses which of its tools to offer.")}
      onSubmit={() => save.mutate()}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button variant="primary" loading={save.isPending} onClick={() => save.mutate()}>
            {t("common.save", "Save")}
          </Button>
        </>
      }
    >
      <CheckboxGroup aria-label={t("common.projects", "Projects")} value={chosen} onValueChange={setChosen}>
        {projects.map((p) => (
          <Checkbox key={p.id} value={p.id} label={p.name} />
        ))}
      </CheckboxGroup>
    </Dialog>
  );
}

function ReplaceSecretDialog({ org, server, open, onClose }: { org: string; server: McpServerView; open: boolean; onClose: () => void }) {
  const [name, setName] = useState(server.connection?.header_name ?? "Authorization");
  const [value, setValue] = useState("");
  const save = useApiMutation({
    mutationFn: async () =>
      data(
        await api.POST("/platform/v1/orgs/{org}/mcp-servers/{server}/connection", {
          params: { path: { org, server: server.id } },
          body: { header: { name: name.trim(), value: value.trim() } },
        }),
      ),
    invalidates: [SERVERS(org)],
    onSuccess: (answer) => {
      setValue("");
      toast.success(t("orgHome.mcp.secretReplaced", "The secret of {name} was replaced", { name: answer.server.name }));
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
      open={open}
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={t("orgHome.mcp.replaceTitle", "Replace the secret of {name}", { name: server.name })}
      description={t("orgHome.mcp.replaceHint", "Kaeso sends the new header with every call from now on.")}
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
            {t("orgHome.mcp.replaceSecret", "Replace secret")}
          </Button>
        </>
      }
    >
      <HeaderFields name={name} value={value} onName={setName} onValue={setValue} />
    </Dialog>
  );
}

function DeleteServerDialog({
  org,
  server,
  open,
  onOpenChange,
  onDeleted,
}: {
  org: string;
  server: McpServerView;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDeleted: () => void;
}) {
  const remove = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/orgs/{org}/mcp-servers/{server}", { params: { path: { org, server: server.id }, header: ifMatch(server) } });
    },
    invalidates: [SERVERS(org)],
    onSuccess: () => {
      onOpenChange(false);
      toast(t("orgHome.mcp.deleted", "{name} deleted", { name: server.name }));
      onDeleted();
    },
    onError: () => onOpenChange(false),
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("orgHome.mcp.deleteQuestion", "Delete {name}?", { name: server.name })}
      description={
        server.projects.length
          ? t("orgHome.mcp.deleteUsed", "{count, plural, one {# project stops} other {# projects stop}} offering its tools, and its sign-ins are removed.", {
              count: server.projects.length,
            })
          : t("orgHome.mcp.deleteUnused", "No project uses it; its sign-ins are removed.")
      }
      confirmLabel={t("orgHome.mcp.delete", "Delete server")}
      loading={remove.isPending}
      onConfirm={() => remove.mutateAsync()}
    />
  );
}
