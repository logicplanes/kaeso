import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AlertDialog, Banner, Button, GrantSummary, Input, Spinner, Text } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import { fieldError } from "~/api/field-error";
import type { AgentCatalogue, AgentRequestView, Grantable, PortalRequestView } from "~/api/generated/types";
import { agentCatalogueQuery, grantableQuery } from "~/api/queries/agents";
import { forgetFragment, takeFragment } from "~/auth/fragment";
import { withSignInAgain } from "~/auth/mutation";
import { RecentSignInHost } from "~/auth/recent-sign-in";
import { sessionQuery } from "~/auth/session";
import { AGENT_REQUEST_KEY } from "~/auth/user-data";
import { t } from "~/i18n";
import { AgentGrantEditor, PresetPicker } from "~/kaeso/agent-grant-editor";
import { DEFAULT_PRESET, gainedSensitive, presetDraft, rightLabels, summaryLines, toGrant, type AgentDraft } from "~/kaeso/agent-grant";
import { PublicPage } from "~/shell/public-page";
import { usePageTitle } from "~/shell/title";
import { readStored, writeStored } from "~/storage";
import { useSubmit } from "./form";

function requestToken(): string | null {
  return takeFragment("request") ?? readStored(AGENT_REQUEST_KEY, "session");
}

function forget(token: string) {
  writeStored(AGENT_REQUEST_KEY, null, "session");
  forgetFragment(token);
}

const REFUSED_REQUESTS = ["unsupported_response_type", "invalid_request", "invalid_scope", "invalid_target"];

function linkError(error: string | null): string | undefined {
  if (error === "invalid_client") return t("public.agentLinkUnknownClient", "Kaeso doesn't know this agent; connect it again from the agent.");
  if (error === "invalid_redirect_uri") return t("public.agentLinkBadReturn", "The agent asked to return to an address it didn't register; connect it again.");
  if (error && REFUSED_REQUESTS.includes(error))
    return t("public.agentLinkRefused", "The agent asked for a sign-in Kaeso doesn't offer; update the agent and connect it again.");
  if (error === "temporarily_unavailable")
    return t("public.agentLinkThrottled", "Too many sign-ins came from your address; wait a few minutes, then start again from the agent.");
  if (error === "server_error") return t("public.agentLinkFailed", "Kaeso couldn't start this sign-in; try again from the agent.");
  return undefined;
}

export function AuthorizePage() {
  usePageTitle(t("public.connectAgent", "Connect an agent"));
  const [error] = useState(() => takeFragment("error"));
  const [token] = useState(requestToken);
  if (error || !token) {
    return (
      <PublicPage
        title={t("public.agentLinkUnusable", "This sign-in link can't be used")}
        description={linkError(error) ?? t("public.agentLinkMissing", "Start the sign-in from the agent again; it opens this page with its request.")}
      >
        <Button size="xl" fullWidth href="/">
          {t("public.openKaeso", "Open Kaeso")}
        </Button>
      </PublicPage>
    );
  }
  return <SignedInFirst token={token} />;
}

function SignedInFirst({ token }: { token: string }) {
  const navigate = useNavigate();
  const session = useQuery({ ...sessionQuery, retry: false });
  const signedOut = session.isError || (session.isSuccess && !session.data.user);
  useEffect(() => {
    if (!signedOut) return;
    writeStored(AGENT_REQUEST_KEY, token, "session");
    void navigate({ to: "/sign-in", search: { next: "/authorize" }, replace: true });
  }, [signedOut, navigate, token]);
  if (!session.data?.user) return <Opening />;
  return <Consent token={token} account={session.data.user.email ?? session.data.user.name} />;
}

function Opening() {
  return (
    <PublicPage title={t("public.openingAgentRequest", "Opening the agent's request…")}>
      <div className="flex justify-center show-after-delay">
        <Spinner label={t("public.lookingUpAgentRequest", "Looking up the agent's request")} />
      </div>
    </PublicPage>
  );
}

function Consent({ token, account }: { token: string; account: string }) {
  const request = useQuery({
    queryKey: ["agent-request", token],
    retry: false,
    queryFn: async (): Promise<AgentRequestView> => data(await api.POST("/platform/v1/me/agent-requests/view", { body: { request: token } })),
  });
  const cat = useQuery(agentCatalogueQuery);
  const grantable = useQuery(grantableQuery);
  const ended = request.isError && isApiError(request.error) && request.error.type === "not_found";
  useEffect(() => {
    if (ended) forget(token);
  }, [ended, token]);
  if (ended) {
    return (
      <PublicPage
        title={t("public.agentRequestEnded", "This request has ended")}
        description={t("public.agentRequestEndedHint", "It was used or is older than 15 minutes; connect the agent again.")}
      >
        <Button size="xl" fullWidth href="/">
          {t("public.openKaeso", "Open Kaeso")}
        </Button>
      </PublicPage>
    );
  }
  if (request.data?.portal) return <PortalConsent token={token} account={account} request={request.data} portal={request.data.portal} />;
  if (request.isError || cat.isError || grantable.isError) {
    return (
      <PublicPage
        title={t("public.agentRequestFailed", "The request couldn't be opened")}
        description={isApiError(request.error) ? request.error.message : t("common.unreachable", "Can't reach the gateway; try again.")}
      >
        <Button variant="primary" size="xl" fullWidth onClick={() => void Promise.all([request.refetch(), cat.refetch(), grantable.refetch()])}>
          {t("common.tryAgain", "Try again")}
        </Button>
      </PublicPage>
    );
  }
  if (!request.data || !cat.data || !grantable.data) return <Opening />;
  return <ConsentForm token={token} account={account} request={request.data} cat={cat.data} grantable={grantable.data} />;
}

function Leaving({ host }: { host: string }) {
  return (
    <PublicPage title={t("public.backToAgent", "Back to the agent…")}>
      <div className="flex justify-center show-after-delay">
        <Spinner label={t("public.returningToAgent", "Returning to {host}", { host })} />
      </div>
    </PublicPage>
  );
}

function PortalConsent({ token, account, request, portal }: { token: string; account: string; request: AgentRequestView; portal: PortalRequestView }) {
  const [leaving, setLeaving] = useState(false);
  const { busy, submit } = useSubmit();
  const leave = (to: string) => {
    setLeaving(true);
    forget(token);
    window.location.assign(to);
  };
  const connect = () =>
    submit(async () => {
      const result = data(await api.POST("/platform/v1/me/agent-requests/approve-portal", { body: { request: token } }));
      leave(result.redirect_to);
    });
  const deny = () =>
    submit(async () => {
      const result = data(await api.POST("/platform/v1/me/agent-requests/deny", { body: { request: token } }));
      leave(result.redirect_to);
    });
  if (leaving) return <Leaving host={request.redirect_host} />;
  const toolCount = portal.servers.reduce((n, s) => n + s.tools.length, 0);
  return (
    <PublicPage
      title={t("public.connectPortalTitle", "Connect {client} to {project}", { client: request.client, project: portal.project_name })}
      description={t("public.connectPortalLead", "It calls this project's tools as you, with your access to them.")}
      footer={
        <Text variant="small" tone="tertiary" className="text-center">
          {t("public.agentSignedInAs", "Signed in as {account} · returns to {host}", { account, host: request.redirect_host })}
        </Text>
      }
    >
      <div className="flex flex-col gap-16">
        {portal.local ? (
          <Banner tone="warning" title={t("public.portalLocalTitle", "It returns to this computer")}>
            {t("public.portalLocal", "Continue only if you started this from an app on this device.")}
          </Banner>
        ) : null}
        <GrantSummary
          aria-label={t("public.portalTools", "Tools it can call")}
          {...(toolCount > 0
            ? {
                sections: portal.servers.map((s) => ({
                  id: s.server,
                  title: s.name,
                  hint: t("public.portalServerTools", "{count, plural, one {# tool} other {# tools}}", { count: s.tools.length }),
                  items: s.tools,
                  defaultOpen: portal.servers.length === 1,
                })),
              }
            : { items: [t("public.portalNoTools", "None yet; it gets the tools a project admin turns on.")] })}
          footer={
            toolCount > 0
              ? t("public.portalToolsUntil", "{count, plural, one {# tool} other {# tools}} in {org} › {project}, until you or a project admin disconnect it", {
                  count: toolCount,
                  org: portal.org_name,
                  project: portal.project_name,
                })
              : t("public.portalUntilDisconnected", "In {org} › {project}, until you or a project admin disconnect it", {
                  org: portal.org_name,
                  project: portal.project_name,
                })
          }
        />
        {portal.client_host ? (
          <Text variant="small" tone="secondary">
            {t("public.portalClientHost", "{host} publishes this client's name and return addresses.", { host: portal.client_host })}
          </Text>
        ) : null}
        {portal.connect.length > 0 && (
          <div className="flex flex-col gap-8">
            <Text variant="small" tone="secondary">
              {t("public.portalConnect", "These servers need your own account; their tools fail for you until you connect.")}
            </Text>
            {portal.connect.map((c) => (
              <span key={c.server} className="flex items-center justify-between gap-12">
                <Text>{c.name}</Text>
                <Button size="sm" href={`/settings/account/connections?server=${c.server}`} target="_blank">
                  {t("public.portalConnectServer", "Connect")}
                </Button>
              </span>
            ))}
          </div>
        )}
      </div>
      <div className="mt-8 flex flex-col gap-8">
        <Button variant="primary" size="xl" fullWidth loading={busy} disabled={busy} onClick={() => void connect()}>
          {t("public.connectPortal", "Connect")}
        </Button>
        <Button variant="ghost" size="xl" fullWidth disabled={busy} onClick={() => void deny()}>
          {t("public.denyAgent", "Deny")}
        </Button>
      </div>
    </PublicPage>
  );
}

function ConsentForm({
  token,
  account,
  request,
  cat,
  grantable,
}: {
  token: string;
  account: string;
  request: AgentRequestView;
  cat: AgentCatalogue;
  grantable: Grantable;
}) {
  const [name, setName] = useState(request.client);
  const [customizing, setCustomizing] = useState(false);
  const [draft, setDraft] = useState<AgentDraft>(() => presetDraft(cat, grantable, DEFAULT_PRESET));
  const [confirming, setConfirming] = useState<string[] | null>(null);
  const [leaving, setLeaving] = useState(false);
  const { busy, error, submit } = useSubmit();

  const leave = (to: string) => {
    setLeaving(true);
    forget(token);
    window.location.assign(to);
  };
  const authorize = () =>
    submit(async () => {
      const result = await withSignInAgain(async () =>
        data(
          await api.POST("/platform/v1/me/agent-requests/approve", {
            body: { request: token, name: name.trim(), grant: toGrant(draft) },
          }),
        ),
      );
      leave(result.redirect_to);
    });
  const deny = () =>
    submit(async () => {
      const result = data(await api.POST("/platform/v1/me/agent-requests/deny", { body: { request: token } }));
      leave(result.redirect_to);
    });
  const askFirst = () => {
    const sensitive = gainedSensitive(cat, null, draft);
    if (sensitive.length || draft.preset === "full") setConfirming(sensitive);
    else void authorize();
  };

  if (leaving) return <Leaving host={request.redirect_host} />;
  const shownName = name.trim() || request.client;
  return (
    <PublicPage
      title={t("public.connectAgentTitle", "Connect {name}", { name: shownName })}
      description={t("public.connectAgentLead", "It acts for you in Kaeso, only with the rights you give it.")}
      footer={
        <Text variant="small" tone="tertiary" className="text-center">
          {t("public.agentSignedInAs", "Signed in as {account} · returns to {host}", { account, host: request.redirect_host })}
        </Text>
      }
    >
      <PresetPicker cat={cat} grantable={grantable} value={draft} onChange={setDraft} />
      <GrantSummary
        aria-label={t("public.agentCan", "What it can do")}
        items={summaryLines(cat, grantable, draft)}
        footer={t("public.agentUntilRevoked", "Until you revoke it")}
        action={
          <Button size="sm" variant="secondary" onClick={() => setCustomizing(true)}>
            {t("kaeso.rights.customize", "Customize")}
          </Button>
        }
      />
      <AgentGrantEditor
        cat={cat}
        grantable={grantable}
        value={draft}
        onChange={setDraft}
        open={customizing}
        onOpenChange={setCustomizing}
        title={t("kaeso.agents.customizeTitle", "Customize {name}", { name: shownName })}
        description={t("public.customizeAgentHint", "Nothing changes until you authorize.")}
      >
        <Input label={t("common.name", "Name")} value={name} onChange={(e) => setName(e.target.value)} maxLength={100} error={fieldError(error, "name")} />
      </AgentGrantEditor>
      <div className="mt-8 flex flex-col gap-8">
        <Button variant="primary" size="xl" fullWidth loading={busy} disabled={!name.trim() || busy} onClick={askFirst}>
          {t("public.authorizeAgent", "Authorize")}
        </Button>
        <Button variant="ghost" size="xl" fullWidth disabled={busy} onClick={() => void deny()}>
          {t("public.denyAgent", "Deny")}
        </Button>
      </div>
      <RecentSignInHost />
      <AlertDialog
        open={confirming !== null}
        onOpenChange={(o) => !o && setConfirming(null)}
        tone="danger"
        title={t("public.confirmAgentRights", "Give this agent sensitive rights?")}
        description={
          confirming?.length
            ? t("public.confirmAgentRightsList", "It would get {rights}; you may need to sign in again first.", {
                rights: rightLabels(cat, confirming),
              })
            : t("public.confirmFullAccess", "It could do everything you can that agents may do; you may need to sign in again first.")
        }
        confirmLabel={t("public.authorizeAgent", "Authorize")}
        onConfirm={() => {
          setConfirming(null);
          void authorize();
        }}
      />
    </PublicPage>
  );
}
