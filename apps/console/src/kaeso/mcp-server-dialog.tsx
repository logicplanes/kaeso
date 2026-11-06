import { useState, type ComponentProps } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Badge,
  Banner,
  Button,
  Checkbox,
  CheckboxGroup,
  Code,
  Input,
  KeyValue,
  Link,
  List,
  Loader,
  Radio,
  RadioGroup,
  Segmented,
  Skeleton,
  Status,
  StepDialog,
  Switch,
  Text,
  icons,
} from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { McpHeader, McpServerCreate, McpServerProbeView, McpServerView } from "~/api/generated/types";
import { mcpServerQuery, mcpServerToolsQuery } from "~/api/queries/mcp-servers";
import { projectToolViews } from "~/api/queries/invalidation";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";
import { HeaderFields } from "./mcp-header-fields";
import { HANDLE, MAX_HANDLE, toolPrefix, connectionWord, fatalProblem, hint, hostOf, problemText, serverInfoText, signInLabel } from "./mcp-servers";
import { useMcpSignIn } from "./mcp-sign-in";

type DialogStep = ComponentProps<typeof StepDialog>["steps"][number];
type Method = "none" | "header" | "oauth";
type Mode = "shared" | "per_user";
type Choice = { name: string; title?: string | null | undefined; description?: string | null | undefined; annotations?: unknown };

const nameFrom = (probe: McpServerProbeView): string => {
  const info = probe.server_info as Record<string, unknown> | null | undefined;
  const named = typeof info?.title === "string" ? info.title : typeof info?.name === "string" ? info.name : null;
  return (named ?? hostOf(probe.url).replace(/^mcp\./, "")).slice(0, 100);
};

export function AddServerDialog({ org, open, onClose, onAdded }: { org: string; open: boolean; onClose: () => void; onAdded: (s: McpServerView) => void }) {
  const projects = useQuery(orgProjectsQuery(org));
  const [url, setUrl] = useState("");
  const [probe, setProbe] = useState<{ url: string; view: McpServerProbeView } | null>(null);
  const [name, setName] = useState("");
  const [handle, setHandle] = useState("");
  const [method, setMethod] = useState<Method>("none");
  const [mode, setMode] = useState<Mode>("shared");
  const [headerName, setHeaderName] = useState("Authorization");
  const [headerValue, setHeaderValue] = useState("");
  const [headerProblem, setHeaderProblem] = useState<string | null>(null);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [created, setCreated] = useState<McpServerView | null>(null);
  const [listed, setListed] = useState<Choice[] | null>(null);
  const [chosen, setChosen] = useState<Set<string> | null>(null);
  const [hold, setHold] = useState(true);
  const [picked, setPicked] = useState<string[]>([]);
  const signIn = useMcpSignIn(org);

  const shared = method !== "none" && mode === "shared";
  const connected = method === "oauth" && shared ? signIn.server : null;
  const watched = useQuery({
    ...mcpServerQuery(org, connected?.id ?? ""),
    enabled: !!connected,
    refetchInterval: (q) => (q.state.data?.status === "unchecked" ? 2000 : false),
  });
  const synced = !!watched.data && watched.data.status !== "unchecked";
  const serverTools = useQuery({ ...mcpServerToolsQuery(org, connected?.id ?? ""), enabled: synced });
  const choices: Choice[] | null =
    method === "oauth" ? (connected ? (serverTools.data?.filter((x) => x.state !== "removed") ?? []) : null) : shared || method === "none" ? listed : null;
  const selected = chosen ?? new Set((choices ?? []).filter((c) => hint(c.annotations, "readOnlyHint")).map((c) => c.name));

  const checked = probe?.url === url.trim() ? probe.view : null;
  const offer = checked?.sign_in ?? null;
  const needsClient = method === "oauth" && !!offer && offer.method === "oauth" && !offer.client_metadata_documents && !offer.registration;
  const header = (): McpHeader => ({ name: headerName.trim(), value: headerValue.trim() });

  const check = useApiMutation({
    mutationFn: async (withHeader: McpHeader | null) =>
      data(
        await api.POST("/platform/v1/orgs/{org}/mcp-servers/probe", {
          params: { path: { org } },
          body: { url: url.trim(), ...(withHeader ? { header: withHeader } : {}) },
        }),
      ),
  });
  const body = (): McpServerCreate => ({
    handle,
    name: name.trim(),
    url: url.trim(),
    auth_method: method,
    credential_mode: method === "none" ? "shared" : mode,
    change_policy: hold ? "hold" : "accept",
    projects: picked,
    ...(needsClient && clientId.trim()
      ? { oauth_client: { client_id: clientId.trim(), ...(clientSecret.trim() ? { client_secret: clientSecret.trim() } : {}) } }
      : {}),
  });
  const ensure = async (): Promise<string> => {
    if (created) return created.id;
    const s = data(await api.POST("/platform/v1/orgs/{org}/mcp-servers", { params: { path: { org } }, body: { ...body(), projects: [] } }));
    setCreated(s);
    return s.id;
  };
  const save = useApiMutation({
    mutationFn: async () => {
      let server = created;
      if (!server) {
        server = data(await api.POST("/platform/v1/orgs/{org}/mcp-servers", { params: { path: { org } }, body: body() }));
        setCreated(server);
      } else {
        server = data(
          await api.PATCH("/platform/v1/orgs/{org}/mcp-servers/{server}", {
            params: { path: { org, server: server.id } },
            body: {
              ...(name.trim() !== server.name ? { name: name.trim() } : {}),
              ...(handle !== server.handle ? { handle } : {}),
              change_policy: hold ? "hold" : "accept",
              projects: picked,
            },
          }),
        );
      }
      if (method === "header" && shared && !server.connection)
        server = data(
          await api.POST("/platform/v1/orgs/{org}/mcp-servers/{server}/connection", {
            params: { path: { org, server: server.id } },
            body: { header: header() },
          }),
        ).server;
      if (choices && selected.size && picked.length) {
        const tools = data(await api.GET("/platform/v1/orgs/{org}/mcp-servers/{server}/tools", { params: { path: { org, server: server.id } } })).data;
        const on = tools.filter((x) => selected.has(x.name)).map((x) => ({ id: x.id, enabled: true }));
        for (const project of picked)
          await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/tools", { params: { path: { org, project } }, body: { tools: on } });
      }
      return server;
    },
    invalidates: [["orgs", org, "mcp-servers"], projectToolViews(org)],
    onSuccess: (s) => {
      setHeaderValue("");
      toast.success(t("orgHome.mcp.added", "{name} added", { name: s.name }));
      onAdded(s);
      onClose();
    },
  });

  const addressBlocked = !url.trim()
    ? t("orgHome.mcp.addressNeeded", "Enter the server's address")
    : checked && fatalProblem(checked.problem)
      ? t("orgHome.mcp.addressUnusable", "Kaeso can't use this address")
      : checked && !name.trim()
        ? t("orgHome.mcp.nameNeeded", "Give the server a name")
        : checked && (!HANDLE.test(handle) || handle.length > MAX_HANDLE)
          ? t("orgHome.mcp.handleNeeded", "Use 1 to 24 lowercase letters, digits and hyphens")
          : undefined;
  const problem = checked ? problemText(checked) : null;
  const info = checked ? serverInfoText(checked.server_info) : null;
  const addressStep: DialogStep = {
    id: "address",
    title: t("orgHome.mcp.addressTitle", "Where is the server?"),
    description: t("orgHome.mcp.addressDescription", "Its Streamable HTTP address, such as https://mcp.example.com/mcp."),
    blockedReason: addressBlocked,
    ...(checked ? {} : { continueLabel: t("orgHome.mcp.checkAddress", "Check address") }),
    onContinue: async () => {
      if (checked) return true;
      try {
        const view = await check.mutateAsync(null);
        setProbe({ url: url.trim(), view });
        if (!name.trim()) setName(nameFrom(view));
        if (!handle) setHandle(view.handle);
        setMethod(view.sign_in?.method ?? (view.problem === "sign_in" ? "header" : "none"));
        setListed(view.tools ?? null);
        setChosen(null);
      } catch {
        return false;
      }
      return false;
    },
    content: (
      <div className="flex flex-col gap-16">
        <Input
          label={t("orgHome.mcp.address", "Address")}
          placeholder={t("orgHome.mcp.addressPlaceholder", "https://mcp.example.com/mcp")}
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          disabled={!!created}
          spellCheck={false}
          autoComplete="off"
          inputMode="url"
          autoFocus
        />
        {check.isPending && <Loader label={t("orgHome.mcp.checking", "Checking the address…")} />}
        {checked && problem && <Banner tone={fatalProblem(checked.problem) ? "danger" : "warning"}>{problem}</Banner>}
        {checked && !fatalProblem(checked.problem) && (
          <>
            <KeyValue
              items={[
                ...(info ? [{ label: t("orgHome.mcp.serverInfo", "Says it is"), value: info }] : []),
                ...(checked.protocol_version ? [{ label: t("orgHome.mcp.protocol", "MCP version"), value: checked.protocol_version, mono: true }] : []),
                { label: t("orgHome.mcp.signInMethod", "Sign-in"), value: signInLabel(checked.sign_in?.method ?? "none") },
                {
                  label: t("orgHome.mcp.tools", "Tools"),
                  value: checked.tools
                    ? t("orgHome.mcp.toolCount", "{count, plural, one {# tool} other {# tools}}", { count: checked.tools.length })
                    : t("orgHome.mcp.toolsAfterSignIn", "Listed after the sign-in"),
                },
              ]}
            />
            <Input label={t("common.name", "Name")} value={name} onChange={(e) => setName(e.target.value)} maxLength={100} />
            <Input
              label={t("orgHome.mcp.handle", "Handle")}
              description={t("orgHome.mcp.handleHint", "Its tools are named {prefix}tool outside Kaeso.", { prefix: toolPrefix(handle) })}
              value={handle}
              onChange={(e) => setHandle(e.target.value.toLowerCase())}
              maxLength={MAX_HANDLE}
              spellCheck={false}
              autoComplete="off"
            />
          </>
        )}
      </div>
    ),
  };

  const signInBlocked =
    method === "header" && shared
      ? !headerName.trim()
        ? t("orgHome.mcp.headerNameNeeded", "Name the header")
        : !headerValue.trim()
          ? t("orgHome.mcp.headerValueNeeded", "Paste the header's value")
          : undefined
      : method === "oauth" && shared && !connected
        ? needsClient && !clientId.trim() && !created
          ? t("orgHome.mcp.clientNeeded", "Paste the client id")
          : t("orgHome.mcp.signInFirst", "Sign in first")
        : undefined;
  const connectedWord = connected ? connectionWord(connected) : null;
  const signInStep: DialogStep = {
    id: "sign-in",
    title: t("orgHome.mcp.signInTitle", "How does Kaeso sign in?"),
    description: t("orgHome.mcp.signInDescription", "Kaeso keeps secrets sealed and never shows them again."),
    blockedReason: signInBlocked,
    onContinue: async () => {
      if (!(method === "header" && shared)) return true;
      try {
        const view = await check.mutateAsync(header());
        const refused = problemText(view);
        setHeaderProblem(refused);
        if (refused) return false;
        setListed(view.tools ?? []);
        setChosen(null);
        return true;
      } catch {
        return false;
      }
    },
    content: (
      <div className="flex flex-col gap-16">
        <RadioGroup
          variant="cards"
          aria-label={t("orgHome.mcp.signInMethod", "Sign-in")}
          value={method}
          disabled={!!created}
          onValueChange={(v) => {
            setMethod(v === "header" || v === "oauth" ? v : "none");
            setHeaderProblem(null);
          }}
        >
          <Radio value="none" label={signInLabel("none")} description={t("orgHome.mcp.noneHint", "The server answers anyone who has its address.")} />
          <Radio
            value="header"
            label={signInLabel("header")}
            description={t("orgHome.mcp.headerHint", "Kaeso sends a header, such as an API token, with every call.")}
          />
          <Radio
            value="oauth"
            label={signInLabel("oauth")}
            description={t("orgHome.mcp.oauthHint", "Sign in with an account at the server, like an app would.")}
          />
        </RadioGroup>
        {method !== "none" && (
          <Segmented
            aria-label={t("orgHome.mcp.whoseAccount", "Whose account")}
            value={mode}
            disabled={!!created}
            onValueChange={(v) => setMode(v === "per_user" ? "per_user" : "shared")}
            options={[
              { value: "shared", label: t("orgHome.mcp.mode.shared", "Shared") },
              { value: "per_user", label: t("orgHome.mcp.mode.perUser", "Each person signs in") },
            ]}
          />
        )}
        {method !== "none" && mode === "per_user" && (
          <Text variant="small" tone="secondary">
            {t("orgHome.mcp.perUserHint", "Each person connects their own account under Account › Connections.")}
          </Text>
        )}
        {method === "header" && shared && (
          <>
            <HeaderFields name={headerName} value={headerValue} onName={setHeaderName} onValue={setHeaderValue} />
            {headerProblem && <Banner tone="danger">{headerProblem}</Banner>}
          </>
        )}
        {method === "oauth" && shared && (
          <div className="flex flex-col items-start gap-12">
            {needsClient && !created && (
              <>
                <Text variant="small" tone="secondary">
                  {t("orgHome.mcp.clientHint", "This server needs a client registered by hand at its sign-in.")}
                </Text>
                <Input
                  className="w-full"
                  label={t("orgHome.mcp.clientId", "Client id")}
                  value={clientId}
                  onChange={(e) => setClientId(e.target.value)}
                  autoComplete="off"
                />
                <Input
                  className="w-full"
                  secret
                  label={t("orgHome.mcp.clientSecret", "Client secret")}
                  description={t("orgHome.mcp.clientSecretHint", "Leave it empty for a public client.")}
                  value={clientSecret}
                  onChange={(e) => setClientSecret(e.target.value)}
                  autoComplete="off"
                />
              </>
            )}
            {connectedWord ? (
              <Status tone={connectedWord.tone}>{connectedWord.label}</Status>
            ) : (
              <Button
                leadingIcon={icons.openExternal}
                loading={signIn.starting}
                disabled={needsClient && !clientId.trim() && !created}
                {...(needsClient && !clientId.trim() && !created ? { disabledReason: t("orgHome.mcp.clientNeeded", "Paste the client id") } : {})}
                onClick={() => signIn.start(ensure)}
              >
                {t("orgHome.mcp.signInTo", "Sign in to {name}", { name: name.trim() })}
              </Button>
            )}
            {signIn.waiting && <Loader label={t("orgHome.mcp.waitingForSignIn", "Waiting for the sign-in in the new tab…")} />}
            {signIn.link && (
              <Link href={signIn.link} external>
                {t("orgHome.mcp.openSignIn", "Open the sign-in page")}
              </Link>
            )}
            {signIn.expired && (
              <Text variant="small" tone="secondary">
                {t("orgHome.mcp.signInExpired", "The sign-in took too long; start it again.")}
              </Text>
            )}
          </div>
        )}
      </div>
    ),
  };

  const toolsStep: DialogStep = {
    id: "tools",
    title: t("orgHome.mcp.toolsTitle", "Which tools should projects offer?"),
    description: t("orgHome.mcp.toolsDescription", "Read-only tools are chosen; each project can change this later."),
    content: (
      <div className="flex flex-col gap-16">
        {!choices || (method === "oauth" && !serverTools.data) ? (
          <Loader label={t("orgHome.mcp.listing", "Listing the server's tools…")} />
        ) : (
          <List
            aria-label={t("orgHome.mcp.tools", "Tools")}
            selectable
            selectionBar={false}
            selected={selected}
            onSelectedChange={setChosen}
            empty={<Text tone="tertiary">{t("orgHome.mcp.noToolsListed", "The server lists no tools.")}</Text>}
            items={choices.map((c) => ({
              id: c.name,
              textValue: c.name,
              title: <Code>{c.name}</Code>,
              subtitle: c.title || c.description || undefined,
              ...(hint(c.annotations, "destructiveHint") ? { meta: <Badge tone="warning">{t("orgHome.mcp.destructive", "Destructive")}</Badge> } : {}),
            }))}
          />
        )}
        <Switch
          label={t("orgHome.mcp.holdChanges", "Hold changed tools for review")}
          description={t("orgHome.mcp.holdChangesHint", "A changed tool stays off until someone approves it.")}
          checked={hold}
          onCheckedChange={setHold}
        />
      </div>
    ),
  };

  const options = projects.data ?? [];
  const projectsStep: DialogStep = {
    id: "projects",
    title: t("orgHome.mcp.projectsStepTitle", "Which projects use it?"),
    description: choices
      ? t("orgHome.mcp.projectsStepHint", "Their portals offer the tools you chose.")
      : t("orgHome.mcp.projectsStepLater", "Each project chooses its tools once the server lists them."),
    content: projects.isPending ? (
      <Skeleton lines={3} />
    ) : (
      <CheckboxGroup aria-label={t("common.projects", "Projects")} value={picked} onValueChange={setPicked}>
        {options.map((p) => (
          <Checkbox key={p.id} value={p.id} label={p.name} />
        ))}
      </CheckboxGroup>
    ),
  };

  const steps = [addressStep, signInStep, ...(choices ? [toolsStep] : []), projectsStep];
  return (
    <StepDialog
      size="md"
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={t("orgHome.mcp.addTitle", "Add MCP server")}
      submitLabel={t("orgHome.mcp.add", "Add server")}
      submitting={save.isPending}
      onSubmit={() => save.mutate()}
      steps={steps}
    />
  );
}
