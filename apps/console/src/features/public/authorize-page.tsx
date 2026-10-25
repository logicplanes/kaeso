import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AlertDialog, Button, GrantSummary, Input, Spinner, Text } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import { fieldError } from "~/api/field-error";
import type { AgentCatalogue, AgentRequestView, Grantable } from "~/api/generated/types";
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

function linkError(error: string | null): string | undefined {
  if (error === "invalid_client") return t("public.agentLinkUnknownClient", "Kaeso doesn't know this agent; connect it again from the agent.");
  if (error === "invalid_redirect_uri") return t("public.agentLinkBadReturn", "The agent asked to return to an address it didn't register; connect it again.");
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

  if (leaving) {
    return (
      <PublicPage title={t("public.backToAgent", "Back to the agent…")}>
        <div className="flex justify-center show-after-delay">
          <Spinner label={t("public.returningToAgent", "Returning to {host}", { host: request.redirect_host })} />
        </div>
      </PublicPage>
    );
  }
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
