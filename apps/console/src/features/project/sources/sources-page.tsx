import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  AppFrame,
  Button,
  Checkbox,
  Empty,
  Input,
  List,
  Radio,
  RadioGroup,
  SecretReveal,
  Select,
  SettingsGroup,
  SourceBadge,
  Sparkline,
  Status,
  StepDialog,
  Text,
  icons,
  type StepDialogResult,
} from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { fieldError } from "~/api/field-error";
import type { SourceCreated, SourceKindView } from "~/api/generated/types";
import { serviceAccountsQuery } from "~/api/queries/members";
import { sourcesQuery } from "~/api/queries/sources";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useProjectRights } from "~/auth/rights";
import { LoadFailed } from "~/features/org/load-failed";
import { compareNames } from "~/format/collate";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { ScopeTabs } from "~/kaeso/scope-tabs";
import { patchSearch, type SearchPatch } from "~/search";
import { usePageCreateAction } from "~/shell/create-action";
import { ProjectHeader } from "~/shell/project-header";
import { toast } from "~/shell/toast";
import { SOURCE_KINDS, eventsByDay, kindHint, kindName, statusLabel, statusTone, totals } from "./labels";
import { setupSnippets } from "./setup";
import { SourceSheet } from "./source-sheet";
import { Connection } from "./connection";

export type SourcesSearch = { source?: string; scope?: "archived" | "all"; add?: boolean };

const today = () => new Date().toISOString().slice(0, 10);

export function SourcesPage({ org, project, search }: { org: string; project: string; search: SourcesSearch }) {
  const navigate = useNavigate();
  const needs = useNeedsRight(org, project);
  const rights = useProjectRights(org, project);
  const scope = search.scope ?? "active";
  const sources = useQuery(sourcesQuery(org, project));
  const [adding, setAdding] = useState(!!search.add);
  const canAdd = rights.has("sources.manage");
  const reason = canAdd ? undefined : needs("sources.manage");
  usePageCreateAction({ label: t("project.sources.add", "Add source"), run: () => setAdding(true), disabledReason: reason });

  const setSearch = (patch: SearchPatch<SourcesSearch>) => {
    void navigate({
      to: "/orgs/$org/projects/$project/sources",
      params: { org, project },
      search: patchSearch(search, patch, ["add"]),
      replace: true,
    });
  };

  const list = useMemo(
    () =>
      [...(sources.data ?? [])]
        .filter((s) => (scope === "active" ? !s.archived_at : scope === "archived" ? !!s.archived_at : true))
        .sort((a, b) => compareNames(a.name, b.name)),
    [sources.data, scope],
  );
  const day = today();
  const items = list.map((s) => {
    const sum = totals(s);
    const days = eventsByDay(s, day);
    return {
      id: s.id,
      textValue: s.name,
      title: s.name,
      leading: <SourceBadge kind={s.kind} size="md" />,
      subtitle: s.last_event_at
        ? t("project.sources.lastEvent", "{kind} · last event {when}", { kind: kindName(s.kind), when: formatRelative(s.last_event_at) })
        : kindName(s.kind),
      meta: (
        <span className="flex items-center gap-12">
          {sum.refused > 0 && (
            <Text variant="small" tone="secondary" tabular>
              {t("project.sources.refusedCount", "{count, plural, one {# refused} other {# refused}}", { count: sum.refused })}
            </Text>
          )}
          <Sparkline
            aria-label={t("project.sources.eventsChart", "Events a day over the last 14 days")}
            values={days.map((d) => d.events)}
            labels={days.map((d) => d.day)}
            seriesLabel={t("project.sources.events", "Events")}
          />
          <Status tone={statusTone(s.status)} size="sm">
            {statusLabel(s.status)}
          </Status>
        </span>
      ),
    };
  });

  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("project.sources.title", "Sources")}
        path="sources"
        tabs={<ScopeTabs label={t("project.sources.title", "Sources")} value={scope} onChange={(v) => setSearch({ scope: v === "active" ? undefined : v })} />}
        actions={
          <Button
            size="sm"
            variant="primary"
            leadingIcon={icons.add}
            disabled={!canAdd}
            {...(reason ? { disabledReason: reason } : {})}
            onClick={() => setAdding(true)}
          >
            {t("project.sources.add", "Add source")}
          </Button>
        }
      />
      <AppFrame.Body padding="list">
        {sources.isPending ? (
          <List aria-label={t("project.sources.title", "Sources")} items={[]} loading />
        ) : sources.isError ? (
          <LoadFailed title={t("project.sources.listFailed", "The sources couldn't load")} size="md" />
        ) : list.length === 0 && scope === "archived" ? (
          <Empty size="lg" title={t("project.sources.noneArchived", "No archived sources")} />
        ) : list.length === 0 ? (
          <Empty
            size="lg"
            title={t("project.sources.none", "No sources yet")}
            description={t("project.sources.noneHint", "Count the requests Claude Code, Codex or a gateway make without going through Kaeso.")}
            actions={
              canAdd ? (
                <Button variant="primary" onClick={() => setAdding(true)}>
                  {t("project.sources.add", "Add source")}
                </Button>
              ) : undefined
            }
          />
        ) : (
          <List aria-label={t("project.sources.title", "Sources")} items={items} onOpen={(item) => setSearch({ source: item.id })} />
        )}
      </AppFrame.Body>
      <SourceSheet org={org} project={project} sourceId={search.source} onClose={() => setSearch({ source: undefined })} />
      {adding && (
        <AddSourceDialog
          org={org}
          project={project}
          onClose={() => {
            setAdding(false);
            if (search.add) setSearch({});
          }}
        />
      )}
    </>
  );
}

/** Who a new source's requests count for when an event names nobody Kaeso knows: the person adding it, or a service account. */
const ME = "me";

function AddSourceDialog({ org, project, onClose }: { org: string; project: string; onClose: () => void }) {
  const rights = useProjectRights(org, project);
  const accounts = useQuery({ ...serviceAccountsQuery(org, project), enabled: rights.has("service_accounts.view") });
  const [kind, setKind] = useState<SourceKindView>("claude_code");
  const [name, setName] = useState<string | null>(null);
  const [owner, setOwner] = useState(ME);
  const [matchMembers, setMatchMembers] = useState(true);
  const [content, setContent] = useState(false);
  const [created, setCreated] = useState<SourceCreated | null>(null);
  const shownName = name ?? kindName(kind);
  const create = useApiMutation({
    mutationFn: async () =>
      data(
        await api.POST("/platform/v1/orgs/{org}/projects/{project}/sources", {
          params: { path: { org, project } },
          body: {
            kind,
            name: shownName.trim(),
            match_members: matchMembers,
            content,
            ...(owner === ME ? {} : { service_account_id: owner }),
          },
        }),
      ),
    invalidates: [["orgs", org, "projects", project, "sources"]],
    onSuccess: (c) => {
      toast(t("project.sources.added", "Source {name} added", { name: c.source.name }));
      setCreated(c);
    },
  });
  const owners = [
    { value: ME, label: t("project.sources.countsForMe", "You") },
    ...[...(accounts.data ?? [])]
      .sort((a, b) => compareNames(a.name, b.name))
      .map((a) => ({ value: a.id, label: t("project.sources.countsForAccount", "{name} (service account)", { name: a.name }) })),
  ];
  const result: StepDialogResult | undefined = created
    ? {
        size: "lg",
        title: t("project.sources.connect", "Connect {name}", { name: created.source.name }),
        description: t("project.sources.connectHint", "Copy the key now, it's shown once; put these lines where they go, then use the client."),
        footer: <Button onClick={onClose}>{common.finish()}</Button>,
        content: (
          <div className="flex flex-col gap-24">
            <SecretReveal
              secret={created.key}
              name={created.source.name}
              baseUrl={window.location.origin}
              snippets={setupSnippets({
                kind: created.source.kind,
                ingestUrl: created.source.ingest_url,
                secret: created.key,
                content: created.source.content,
              })}
            />
            <Connection org={org} project={project} source={created.source} />
          </div>
        ),
      }
    : undefined;
  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      result={result}
      title={t("project.sources.add", "Add source")}
      submitLabel={t("project.sources.add", "Add source")}
      submitting={create.isPending}
      onSubmit={() => shownName.trim() && create.mutate()}
      refusedStep={create.error?.param === "name" ? "details" : undefined}
      size="md"
      steps={[
        {
          id: "kind",
          title: t("project.sources.whatReports", "What reports to Kaeso?"),
          description: t("project.sources.whatReportsHint", "Its requests still go straight to their provider; Kaeso only counts and logs them."),
          content: (
            <RadioGroup
              variant="cards"
              aria-label={t("project.sources.kindLabel", "Kind of source")}
              value={kind}
              onValueChange={(v) => setKind(v as SourceKindView)}
            >
              {SOURCE_KINDS.map((k) => (
                <Radio key={k} value={k} label={<SourceBadge kind={k} withName />} description={kindHint(k)} />
              ))}
            </RadioGroup>
          ),
        },
        {
          id: "details",
          title: t("project.sources.whose", "Whose requests are they?"),
          description: t("project.sources.whoseHint", "An event's email address finds the member; the rest count for whom you choose here."),
          blockedReason: shownName.trim() ? undefined : t("project.sources.giveName", "Give it a name"),
          content: (
            <div className="flex flex-col gap-16">
              <Input
                label={common.name()}
                placeholder={t("project.sources.nameExample", "Claude Code, Ada's laptop")}
                value={shownName}
                onChange={(e) => {
                  setName(e.target.value);
                  if (create.isError) create.reset();
                }}
                error={fieldError(create.error, "name")}
                autoFocus
                maxLength={100}
              />
              <Select
                label={t("project.sources.countsFor", "Counts for")}
                description={t("project.sources.countsForHint", "Whom requests count for when an event names nobody Kaeso knows.")}
                options={owners}
                value={owner}
                onValueChange={(v: string) => setOwner(v)}
                fullWidth
              />
              <SettingsGroup aria-label={t("project.sources.options", "Options")}>
                <Checkbox
                  variant="card"
                  label={t("project.sources.matchMembers", "Match members by email")}
                  description={t("project.sources.matchMembersHint", "An event with a member's email address counts for that member.")}
                  checked={matchMembers}
                  onCheckedChange={setMatchMembers}
                />
                <Checkbox
                  variant="card"
                  label={t("project.sources.content", "Keep prompts and replies")}
                  description={t("project.sources.contentHint", "Only what the client sends, and only where the project keeps content.")}
                  checked={content}
                  onCheckedChange={setContent}
                />
              </SettingsGroup>
            </div>
          ),
        },
      ]}
    />
  );
}
