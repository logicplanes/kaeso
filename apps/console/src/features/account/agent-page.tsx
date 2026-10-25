import { useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AgentAvatar, AlertDialog, Banner, Button, Code, Empty, GrantSummary, Input, SettingsGroup, SettingsRow, SettingsSection } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import { fieldError } from "~/api/field-error";
import type { AgentCatalogue, AgentView, Grantable } from "~/api/generated/types";
import { agentActivityQuery, agentCatalogueQuery, agentQuery, grantableQuery } from "~/api/queries/agents";
import { useApiMutation } from "~/auth/mutation";
import { LoadFailed } from "~/features/org/load-failed";
import { t } from "~/i18n";
import { AgentGrantEditor, PresetPicker } from "~/kaeso/agent-grant-editor";
import { fromGrant, gainedSensitive, rightLabels, summaryLines, toGrant, withRequested, type AgentDraft } from "~/kaeso/agent-grant";
import { SettingsLoading, SettingsPage } from "~/shell/settings-page";
import { toast } from "~/shell/toast";
import { agentDates } from "./agents-page";

export interface AgentSearch {
  add?: string;
  org?: string;
  project?: string;
}

const AGENTS = [["me", "agents"]];

export function AgentPage({ agent, search }: { agent: string; search: AgentSearch }) {
  const view = useQuery(agentQuery(agent));
  const cat = useQuery(agentCatalogueQuery);
  const grantable = useQuery(grantableQuery);
  if (view.isError) {
    const gone = isApiError(view.error) && view.error.type === "not_found";
    return (
      <SettingsPage title={t("account.agent", "Agent")} context={t("account.agents", "Agents")}>
        {gone ? (
          <Empty
            title={t("account.agentGone", "This agent was revoked or doesn't exist.")}
            actions={<Button href="/settings/account/agents">{t("account.backToAgents", "Back to agents")}</Button>}
          />
        ) : (
          <LoadFailed title={t("account.agentFailed", "This agent couldn't load")} />
        )}
      </SettingsPage>
    );
  }
  if (!view.data || !cat.data || !grantable.data) {
    return (
      <SettingsPage title={view.data?.name ?? t("account.agent", "Agent")} context={t("account.agents", "Agents")}>
        <SettingsLoading rows={4} />
      </SettingsPage>
    );
  }
  return <Loaded key={view.data.id} agent={view.data} cat={cat.data} grantable={grantable.data} search={search} />;
}

function Loaded({ agent, cat, grantable, search }: { agent: AgentView; cat: AgentCatalogue; grantable: Grantable; search: AgentSearch }) {
  const navigate = useNavigate();
  const saved = fromGrant(agent.grant, grantable);
  const askedFor = rightLabels(cat, [search.add ?? ""]);
  const [requested] = useState(() => (search.add ? withRequested(cat, grantable, saved, search.add, search.org, search.project) : null));
  const [draft, setDraft] = useState<AgentDraft>(requested?.draft ?? saved);
  const [name, setName] = useState(agent.name);
  const [confirming, setConfirming] = useState<string[] | null>(null);
  const [revoking, setRevoking] = useState(false);
  const [customizing, setCustomizing] = useState(() => !!requested);

  const details = useApiMutation({
    mutationFn: async () =>
      data(
        await api.PATCH("/platform/v1/me/agents/{agent}", {
          params: { path: { agent: agent.id } },
          body: { name: name.trim() },
        }),
      ),
    invalidates: AGENTS,
    onSuccess: () => toast(t("account.agentSaved", "Agent saved")),
  });
  const rights = useApiMutation({
    mutationFn: async (d: AgentDraft) =>
      data(await api.PUT("/platform/v1/me/agents/{agent}/rights", { params: { path: { agent: agent.id } }, body: toGrant(d) })),
    invalidates: AGENTS,
    onSuccess: () => {
      toast(t("account.agentRightsSaved", "Rights saved"));
      if (search.add) void navigate({ to: "/settings/account/agents/$agent", params: { agent: agent.id }, search: {}, replace: true });
    },
  });
  const revoke = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/me/agents/{agent}", { params: { path: { agent: agent.id } } });
    },
    invalidates: AGENTS,
    onSuccess: async () => {
      toast(t("account.agentRevoked", "Agent revoked"));
      await navigate({ to: "/settings/account/agents" });
    },
  });

  const changedRights = JSON.stringify(toGrant(draft)) !== JSON.stringify(toGrant(saved));
  const saveRights = () => {
    const sensitive = gainedSensitive(cat, saved, draft);
    if (sensitive.length) setConfirming(sensitive);
    else void rights.mutateAsync(draft);
  };

  return (
    <SettingsPage
      title={agent.name}
      context={t("account.agents", "Agents")}
      description={agentDates(agent)}
      actions={
        <Button size="sm" variant="danger-secondary" onClick={() => setRevoking(true)}>
          {t("account.revokeAgent", "Revoke")}
        </Button>
      }
    >
      {requested && (
        <Banner
          tone={requested.held ? "info" : "warning"}
          title={
            requested.held
              ? t("account.agentAsks", "The agent asks for {right}", { right: askedFor })
              : t("account.agentAsksNotHeld", "The agent asks for {right}, which you don't hold", { right: askedFor })
          }
        >
          {requested.held
            ? t("account.agentAsksHint", "It's added below; save the rights to give it.")
            : t("account.agentAsksNotHeldHint", "Ask an admin for it first; an agent never gets more than you hold.")}
        </Banner>
      )}
      <SettingsSection title={t("account.agentDetails", "Details")}>
        <div className="flex flex-col gap-16">
          <div className="flex items-center gap-12">
            <AgentAvatar model={agent.model} name={name || agent.name} size={40} />
            <Input
              label={t("common.name", "Name")}
              value={name}
              onChange={(e) => setName(e.target.value)}
              maxLength={100}
              error={fieldError(details.error, "name")}
              className="flex-1"
            />
          </div>
          <div>
            <Button
              variant="primary"
              size="sm"
              loading={details.isPending}
              disabled={!name.trim() || name.trim() === agent.name}
              onClick={() => void details.mutateAsync()}
            >
              {t("common.save", "Save")}
            </Button>
          </div>
        </div>
      </SettingsSection>
      <SettingsSection title={t("account.agentRights", "Rights")}>
        <div className="flex flex-col gap-16">
          <PresetPicker cat={cat} grantable={grantable} value={draft} onChange={setDraft} />
          <GrantSummary
            aria-label={t("public.agentCan", "What it can do")}
            items={summaryLines(cat, grantable, draft)}
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
            title={t("kaeso.agents.customizeTitle", "Customize {name}", { name: agent.name })}
            description={t("account.customizeAgentHint", "Nothing changes until you save the rights.")}
            {...(requested ? { start: "organizations" as const } : {})}
          />
          <div className="flex gap-8">
            <Button variant="primary" size="sm" loading={rights.isPending} disabled={!changedRights} onClick={saveRights}>
              {t("account.saveAgentRights", "Save rights")}
            </Button>
            <Button size="sm" disabled={!changedRights || rights.isPending} onClick={() => setDraft(saved)}>
              {t("common.discard", "Discard")}
            </Button>
          </div>
        </div>
      </SettingsSection>
      <Activity agent={agent.id} />
      <AlertDialog
        open={confirming !== null}
        onOpenChange={(o) => !o && setConfirming(null)}
        tone="danger"
        title={t("public.confirmAgentRights", "Give this agent sensitive rights?")}
        description={t("public.confirmAgentRightsList", "It would get {rights}; you may need to sign in again first.", {
          rights: rightLabels(cat, confirming ?? []),
        })}
        confirmLabel={t("account.saveAgentRights", "Save rights")}
        onConfirm={() => {
          setConfirming(null);
          return rights.mutateAsync(draft);
        }}
      />
      <AlertDialog
        open={revoking}
        onOpenChange={setRevoking}
        tone="danger"
        title={t("account.revokeAgentConfirm", "Revoke {name}?", { name: agent.name })}
        description={t("account.revokeAgentConfirmHint", "It's signed out at once and can't act for you until you connect it again.")}
        confirmLabel={t("account.revokeAgent", "Revoke")}
        loading={revoke.isPending}
        onConfirm={() => revoke.mutateAsync()}
      />
    </SettingsPage>
  );
}

const RESULTS: Readonly<Record<string, () => string>> = {
  ok: () => t("account.agentCallOk", "Done"),
  insufficient_rights: () => t("account.agentCallNoRight", "Missing a right"),
};

function Activity({ agent }: { agent: string }) {
  const activity = useInfiniteQuery(agentActivityQuery(agent));
  const calls = activity.data?.pages.flatMap((p) => p.data) ?? [];
  return (
    <SettingsSection title={t("account.agentActivity", "Activity")} description={t("account.agentActivityHint", "Every tool call of the last 90 days.")}>
      {activity.isError ? (
        <LoadFailed title={t("account.agentActivityFailed", "The activity couldn't load")} />
      ) : activity.isPending ? (
        <SettingsLoading rows={3} />
      ) : calls.length === 0 ? (
        <Empty title={t("account.agentNoActivity", "No tool calls yet")} />
      ) : (
        <SettingsGroup>
          {calls.map((c) => (
            <SettingsRow
              key={c.id}
              title={<Code>{c.tool}</Code>}
              description={t("account.agentCall", "{result} · {when} · {address}", {
                result: RESULTS[c.result]?.() ?? c.result,
                when: formatRelative(c.at),
                address: c.ip ?? t("account.unknownAddress", "Unknown address"),
              })}
              value={c.target ?? undefined}
            />
          ))}
        </SettingsGroup>
      )}
      {activity.hasNextPage && (
        <div>
          <Button size="sm" loading={activity.isFetchingNextPage} onClick={() => void activity.fetchNextPage()}>
            {t("common.loadMore", "Load more")}
          </Button>
        </div>
      )}
    </SettingsSection>
  );
}
