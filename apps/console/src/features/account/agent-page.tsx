import { useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AlertDialog, Banner, Button, Code, Empty, GrantSummary, Input, SettingsGroup, SettingsRow, Section, Text } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import { fieldError } from "~/api/field-error";
import type { AgentActivityView, AgentCatalogue, AgentView, Grantable } from "~/api/generated/types";
import { agentActivityQuery, agentCatalogueQuery, agentQuery, grantableQuery } from "~/api/queries/agents";
import { useApiMutation } from "~/auth/mutation";
import { useSession } from "~/auth/session";
import { LoadFailed } from "~/features/org/load-failed";
import { modelLabel } from "~/format/model";
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
  const nameReason = !name.trim()
    ? t("account.enterName", "Enter a name")
    : name.trim() === agent.name
      ? t("account.noChange", "No change to save")
      : undefined;
  const unchanged = changedRights ? {} : { disabledReason: t("account.noChange", "No change to save") };
  const saveRights = () => {
    const sensitive = gainedSensitive(cat, saved, draft);
    if (sensitive.length) setConfirming(sensitive);
    else void rights.mutateAsync(draft);
  };

  return (
    <SettingsPage
      title={agent.name}
      context={t("account.agents", "Agents")}
      description={t("account.agentModelAndDates", "{model} · {dates}", { model: modelLabel(agent.model), dates: agentDates(agent) })}
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
      <Section spacing="settings" title={t("account.agentDetails", "Details")}>
        <SettingsGroup>
          <SettingsRow
            title={t("common.name", "Name")}
            description={t("account.agentNameHint", "Shown in your agent list and the audit log.")}
            control={
              <div className="flex min-w-0 items-start gap-8">
                <div className="w-240 min-w-0">
                  <Input
                    aria-label={t("common.name", "Name")}
                    size="sm"
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && !e.nativeEvent.isComposing && !nameReason && !details.isPending && void details.mutateAsync()}
                    maxLength={100}
                    error={fieldError(details.error, "name")}
                  />
                </div>
                <Button
                  size="sm"
                  loading={details.isPending}
                  disabled={!!nameReason}
                  {...(nameReason ? { disabledReason: nameReason } : {})}
                  onClick={() => void details.mutateAsync()}
                >
                  {t("common.save", "Save")}
                </Button>
              </div>
            }
          />
        </SettingsGroup>
      </Section>
      <Section spacing="settings" title={t("account.agentRights", "Rights")}>
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
            <Button variant="primary" size="sm" loading={rights.isPending} disabled={!changedRights} {...unchanged} onClick={saveRights}>
              {t("account.saveAgentRights", "Save rights")}
            </Button>
            <Button
              size="sm"
              disabled={!changedRights || rights.isPending}
              {...(changedRights ? {} : { disabledReason: t("account.nothingToDiscard", "No change to discard") })}
              onClick={() => setDraft(saved)}
            >
              {t("common.discard", "Discard")}
            </Button>
          </div>
        </div>
      </Section>
      <Activity agent={agent.id} grantable={grantable} />
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

function Activity({ agent, grantable }: { agent: string; grantable: Grantable }) {
  const activity = useInfiniteQuery(agentActivityQuery(agent));
  const session = useSession();
  const calls = activity.data?.pages.flatMap((p) => p.data) ?? [];
  // Where a call acted, by the names the user could open to an agent; an organization they can't any more falls back
  // to their memberships, then to its id. Account and instance tools act nowhere in particular.
  const place = (c: AgentActivityView): string | undefined => {
    if (!c.org_id) return undefined;
    const org = grantable.orgs.find((o) => o.id === c.org_id);
    const orgName = org?.name ?? session.memberships.find((m) => m.org_id === c.org_id)?.org_name ?? c.org_id;
    if (!c.project_id) return orgName;
    const project = org?.projects.find((p) => p.id === c.project_id)?.name ?? c.project_id;
    return t("account.agentCallPlace", "{org} › {project}", { org: orgName, project });
  };
  return (
    <Section
      spacing="settings"
      title={t("account.agentActivity", "Activity")}
      description={t("account.agentActivityHint", "Every tool call of the last 90 days.")}
    >
      {activity.isError ? (
        <LoadFailed title={t("account.agentActivityFailed", "The activity couldn't load")} />
      ) : activity.isPending ? (
        <SettingsLoading rows={3} />
      ) : calls.length === 0 ? (
        <Empty size="md" title={t("account.agentNoActivity", "No tool calls yet")} />
      ) : (
        <SettingsGroup>
          {calls.map((c) => {
            const where = place(c);
            return (
              <SettingsRow
                key={c.id}
                title={
                  <span className="flex flex-wrap items-center gap-8">
                    <Code>{c.tool}</Code>
                    {where && (
                      <Text variant="small" tone="secondary">
                        {where}
                      </Text>
                    )}
                  </span>
                }
                description={t("account.agentCall", "{result} · {when} · {address}", {
                  result: RESULTS[c.result]?.() ?? c.result,
                  when: formatRelative(c.at),
                  address: c.ip ?? t("account.unknownAddress", "Unknown address"),
                })}
                value={c.target ?? undefined}
              />
            );
          })}
        </SettingsGroup>
      )}
      {activity.hasNextPage && (
        <div>
          <Button size="sm" loading={activity.isFetchingNextPage} onClick={() => void activity.fetchNextPage()}>
            {t("common.loadMore", "Load more")}
          </Button>
        </div>
      )}
    </Section>
  );
}
