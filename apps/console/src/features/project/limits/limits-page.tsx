import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  AppFrame,
  Badge,
  Button,
  Combobox,
  Empty,
  Icon,
  List,
  Menu,
  Meter,
  Tabs,
  Segmented,
  Select,
  StepDialog,
  Text,
  Tooltip,
  icons,
  type ListGroup,
} from "@logicplanes/kaeso-ui";
import { limitDescription, limitKindLabel, periodLabel } from "~/format/labels";
import { alertMarks } from "~/format/alert-marks";
import { toast } from "~/shell/toast";
import { api, data } from "~/api/client";
import type { LimitView } from "~/api/generated/types";
import { allKeysQuery } from "~/api/queries/keys";
import { limitsQuery } from "~/api/queries/limits";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { catalogueQuery } from "~/api/queries/models";
import { useApiMutation } from "~/auth/mutation";
import { useOrgCurrency } from "~/kaeso/org-money";
import { LimitEditor, draftOf, limitBody, newLimit, type LimitDraft } from "~/kaeso/limit-editor";
import { EditLimitDialog, RemoveLimitDialog, limitBlocker, useLimitText, useParentLimits, type LimitOwner, type LimitTarget } from "~/kaeso/limits-section";
import { usePageCreateAction } from "~/shell/create-action";
import { ProjectHeader } from "~/shell/project-header";
import { serviceAccountName, usePartyName } from "../keys/party";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { locale, t, type Locale } from "~/i18n";
import { LoadFailed } from "~/features/org/load-failed";
import { common } from "~/i18n/common";
import { Share, lastDays } from "~/kaeso/share";
import { LimitFigure } from "~/kaeso/limit-figure";

type Level = "project" | "member" | "key";
export type LimitsSearch = { level?: Level; sort?: "used"; add?: boolean };

// How full a limit is, or null when its use isn't known.
const share = (l: LimitView): number | null => {
  const amount = Number(l.amount);
  return l.used == null || !(amount > 0) ? null : Number(l.used) / amount;
};

const byUse = (a: LimitView, b: LimitView): number => {
  const x = share(a);
  const y = share(b);
  return x === y ? 0 : x === null ? 1 : y === null ? -1 : y - x;
};

// The member, service account or key a limit applies to, as the add dialog's picker names it.
const subjectValue = (l: LimitView): string =>
  l.subject.type === "user" ? `user:${l.subject.id}` : l.subject.type === "service_account" ? `sa:${l.subject.id}` : l.subject.id;

const LEVEL_IDS: Level[] = ["project", "member", "key"];

const levels = (): { value: Level; label: string; title: string; none: string; empty: string }[] => [
  {
    value: "project",
    label: common.project(),
    title: common.project(),
    none: t("limits.noneProject", "No project limits yet"),
    empty: t("limits.emptyProject", "Add a spend budget or a rate limit for the whole project."),
  },
  {
    value: "member",
    label: common.members(),
    title: t("limits.membersAndAccounts", "Members and service accounts"),
    none: t("limits.noneMember", "No member limits yet"),
    empty: t("limits.emptyMember", "Add a spend budget or a rate limit for a member or a service account."),
  },
  {
    value: "key",
    label: t("limits.keys", "Keys"),
    title: t("limits.keys", "Keys"),
    none: t("limits.noneKey", "No key limits yet"),
    empty: t("limits.emptyKey", "Add a spend budget or a rate limit for one key."),
  },
];

export function LimitsPage({ org, project, search }: { org: string; project: string; search: LimitsSearch }) {
  const needs = useNeedsRight(org, project);
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const limits = useQuery(limitsQuery(org, project));
  const subjectName = useSubjectName(org, project);
  const archivedKey = useArchivedKey(org, project);
  const [adding, setAdding] = useState(!!search.add);
  const [editing, setEditing] = useState<LimitView>();
  const [removing, setRemoving] = useState<LimitView | null>(null);
  const [removeOpen, setRemoveOpen] = useState(false);
  const [copying, setCopying] = useState<LimitView>();
  const canManage = rights.has("limits.manage");
  const reason = canManage ? undefined : needs("limits.manage");
  usePageCreateAction({ label: t("limits.add", "Add limit"), run: () => setAdding(true), disabledReason: reason });

  const go = (next: { level?: Level | undefined; sort?: "used" | undefined }) =>
    void navigate({
      to: "/orgs/$org/projects/$project/limits",
      params: { org, project },
      search: { ...(next.level ? { level: next.level } : {}), ...(next.sort ? { sort: next.sort } : {}) },
      replace: true,
    });
  const setLevel = (level: Level | undefined) => go({ level, sort: search.sort });

  const all = useMemo(() => limits.data ?? [], [limits.data]);
  const levelList = levels();
  const level = levelList.find((l) => l.value === search.level);
  const groups: ListGroup[] = levelList
    .filter((l) => !level || l === level)
    .map((l) => {
      const list = all
        .filter((x) => x.level === l.value)
        .sort((a, b) => (search.sort === "used" ? byUse(a, b) : 0) || Number(archivedKey(a)) - Number(archivedKey(b)));
      return {
        id: l.value,
        title: l.title,
        count: list.length,
        items: list.map((x) => ({
          id: x.id,
          textValue: subjectName(x) ?? limitKindLabel(x.kind),
          glyph: <LimitGlyph org={org} project={project} limit={x} />,
          title: <LimitTitle limit={x} subject={subjectName(x)} archivedKey={archivedKey(x)} />,
          subtitle: <LimitSubtitle org={org} project={project} limit={x} subject={subjectName(x)} />,
          meta: <LimitUse org={org} project={project} limit={x} />,
          date: <LimitResets org={org} project={project} limit={x} />,
          actions: canManage ? (
            <Menu>
              <Menu.Trigger>
                <Button
                  variant="ghost"
                  icon={icons.more}
                  aria-label={t("limits.rowActions", "Actions for the {limit}", { limit: limitDescription(x) })}
                  size="xs"
                />
              </Menu.Trigger>
              <Menu.Content align="end">
                <Menu.Item icon={icons.edit} onClick={() => setEditing(x)}>
                  {common.edit()}
                </Menu.Item>
                {x.level !== "project" && (
                  <Menu.Item icon={icons.copy} onClick={() => setCopying(x)}>
                    {t("limits.copyTo", "Copy to…")}
                  </Menu.Item>
                )}
                <Menu.Item icon={icons.delete} variant="danger" onClick={() => remove(x)}>
                  {common.remove()}
                </Menu.Item>
              </Menu.Content>
            </Menu>
          ) : undefined,
        })),
      };
    })
    .filter((g) => g.items.length > 0);
  const remove = (x: LimitView) => {
    setRemoving(x);
    setRemoveOpen(true);
  };

  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("limits.title", "Limits")}
        path="limits"
        tabs={
          <Tabs variant="pill" value={search.level ?? "all"} onValueChange={(v) => setLevel(v === "all" ? undefined : (v as Level))}>
            <Tabs.List aria-label={t("limits.level", "Level")}>
              <Tabs.Tab value="all">{common.all()}</Tabs.Tab>
              {levelList.map((l) => (
                <Tabs.Tab key={l.value} value={l.value}>
                  {l.label}
                </Tabs.Tab>
              ))}
            </Tabs.List>
          </Tabs>
        }
        actions={
          <span className="flex flex-wrap items-center justify-end gap-8">
            <Select
              aria-label={t("limits.sort", "Sort")}
              size="sm"
              icon={icons.sort}
              value={search.sort ?? "level"}
              onValueChange={(v) => go({ level: search.level, sort: v === "used" ? "used" : undefined })}
              options={[
                { value: "level", label: t("limits.sortLevel", "Level") },
                { value: "used", label: t("limits.sortUsed", "Most used") },
              ]}
            />
            <Share org={org} target={{ kind: "budget", project, ...lastDays(1), costs: rights.has("costs.view") }} />
            <Button
              size="sm"
              variant="primary"
              leadingIcon={icons.add}
              disabled={!canManage}
              {...(reason ? { disabledReason: reason } : {})}
              onClick={() => setAdding(true)}
            >
              {t("limits.add", "Add limit")}
            </Button>
          </span>
        }
      />
      <AppFrame.Body padding="list">
        {limits.isPending ? (
          <List aria-label={t("limits.title", "Limits")} items={[]} loading />
        ) : limits.isError ? (
          <LoadFailed title={t("limits.loadFailed", "The limits couldn't load")} size="md" />
        ) : groups.length === 0 ? (
          <Empty
            size="lg"
            title={level ? level.none : t("limits.none", "No limits yet")}
            description={level ? level.empty : t("limits.emptyAll", "Spend budgets and rate limits; a request must pass every one that applies.")}
            actions={
              canManage ? (
                <Button variant="primary" onClick={() => setAdding(true)}>
                  {t("limits.add", "Add limit")}
                </Button>
              ) : undefined
            }
          />
        ) : (
          <List
            aria-label={t("limits.title", "Limits")}
            groups={groups}
            {...(canManage ? { onOpen: (item: { id: string }) => setEditing(all.find((x) => x.id === item.id)) } : {})}
          />
        )}
      </AppFrame.Body>
      {editing && (
        <EditLimitDialog
          org={org}
          project={project}
          limit={editing}
          onClose={() => setEditing(undefined)}
          onRemove={() => {
            setEditing(undefined);
            remove(editing);
          }}
        />
      )}
      {removing && <RemoveLimitDialog org={org} project={project} limit={removing} open={removeOpen} onOpenChange={setRemoveOpen} />}
      {adding && <AddLimitDialog org={org} project={project} initialLevel={search.level ?? "project"} onClose={() => setAdding(false)} />}
      {copying && (
        <AddLimitDialog
          org={org}
          project={project}
          initialLevel={copying.level as Level}
          copy={{
            draft: draftOf(copying),
            // The gateway allows one limit of a kind, model and period per subject.
            taken: new Set(
              all
                .filter((x) => x.level === copying.level && x.kind === copying.kind && x.model === copying.model && x.period === copying.period)
                .map(subjectValue),
            ),
          }}
          onClose={() => setCopying(undefined)}
        />
      )}
    </>
  );
}

// Floors, not widths: the meters line up across rows, and a longer text pushes its meter over instead of running into it.
const RESETS_WIDTH: Record<Locale, string> = { en: "md:min-w-120", de: "md:min-w-180", "zh-CN": "md:min-w-120" };

const KIND_ICON = { spend: icons.budget, requests: icons.requestLog, tokens: icons.stack } as const;

function LimitGlyph({ org, project, limit }: { org: string; project: string; limit: LimitView }) {
  return <Icon icon={KIND_ICON[useLimitText(org, project, limit).kind]} size="sm" />;
}

function LimitTitle({ limit, subject, archivedKey }: { limit: LimitView; subject: string | undefined; archivedKey: boolean }) {
  return (
    <span className="flex min-w-0 items-center gap-8">
      <Text truncate tone="inherit">
        {subject ?? limitKindLabel(limit.kind)}
      </Text>
      <Badge tone={limit.hard ? "neutral" : "outline"} className="hidden sm:inline-flex">
        {limit.hard ? t("limits.hard", "Hard") : t("limits.soft", "Soft")}
      </Badge>
      {archivedKey && (
        <Tooltip content={t("limits.keyArchivedHint", "The key is archived, so this limit no longer applies to any request.")}>
          <Badge tone="outline">{t("limits.keyArchived", "Key archived")}</Badge>
        </Tooltip>
      )}
    </span>
  );
}

function LimitSubtitle({ org, project, limit, subject }: { org: string; project: string; limit: LimitView; subject: string | undefined }) {
  const text = useLimitText(org, project, limit);
  return (
    <span className="hidden sm:inline">
      {[
        ...(subject ? [limitKindLabel(limit.kind)] : []),
        periodLabel(limit.period),
        text.model,
        ...(text.observed ? [t("limits.observedShort", "{amount} reported by sources", { amount: text.observed })] : []),
      ].join(" · ")}
    </span>
  );
}

function LimitUse({ org, project, limit }: { org: string; project: string; limit: LimitView }) {
  const text = useLimitText(org, project, limit);
  const amount = Number(limit.amount);
  return (
    <span className="flex items-center gap-12">
      <Text variant="small" tone="secondary" tabular className="md:text-right md:whitespace-nowrap">
        <LimitFigure figure={text.value} />
      </Text>
      <Meter
        className="hidden w-120 md:flex"
        value={amount > 0 ? Number(limit.used ?? 0) / amount : 0}
        thresholds={alertMarks(limit)}
        aria-label={t("limits.use", "{kind} use", { kind: limitKindLabel(limit.kind) })}
      />
    </span>
  );
}

function LimitResets({ org, project, limit }: { org: string; project: string; limit: LimitView }) {
  const text = useLimitText(org, project, limit);
  return (
    <span className={`flex justify-end ${RESETS_WIDTH[locale()]}`}>{limit.period === "minute" ? t("limits.lastMinute", "Last minute") : text.resets}</span>
  );
}

function useArchivedKey(org: string, project: string) {
  const rights = useProjectRights(org, project);
  const keys = useQuery({ ...allKeysQuery(org, project, { include_archived: true }), enabled: rights.has("keys.all.view") });
  const archived = new Set((keys.data ?? []).filter((k) => k.state === "archived").map((k) => k.id));
  return (l: LimitView) => l.subject.type === "key" && archived.has(l.subject.id);
}

function useSubjectName(org: string, project: string) {
  const rights = useProjectRights(org, project);
  const keys = useQuery({ ...allKeysQuery(org, project, { include_archived: true }), enabled: rights.has("keys.all.view") });
  const partyName = usePartyName(org, project);
  return (l: LimitView): string | undefined => {
    const { type, id } = l.subject;
    if (type === "project") return undefined;
    if (type === "user") return partyName({ kind: "user", id });
    if (type === "service_account") return serviceAccountName(partyName({ kind: "service_account", id }));
    return keys.data?.find((k) => k.id === id)?.name ?? id;
  };
}

function AddLimitDialog({
  org,
  project,
  initialLevel,
  copy,
  onClose,
}: {
  org: string;
  project: string;
  initialLevel: Level;
  /** Making an existing limit for someone else: its settings, and the subjects that have one like it. */
  copy?: { draft: LimitDraft; taken: ReadonlySet<string> } | undefined;
  onClose: () => void;
}) {
  const rights = useProjectRights(org, project);
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const accounts = useQuery({ ...serviceAccountsQuery(org, project), enabled: rights.has("service_accounts.view") });
  const keys = useQuery({ ...allKeysQuery(org, project), enabled: rights.has("keys.all.view") });
  const catalogue = useQuery(catalogueQuery(org, project));
  const [level, setLevel] = useState<Level>(initialLevel);
  const [subject, setSubject] = useState<string | null>(null);
  const [draft, setDraft] = useState<LimitDraft>(copy?.draft ?? newLimit());
  const free = (o: { value: string }) => !copy?.taken.has(o.value);

  const memberOptions = [
    ...(members.data ?? []).map((m) => ({ value: `user:${m.user.id}`, label: m.user.name, description: m.user.email ?? undefined })),
    ...(accounts.data ?? []).map((a) => ({ value: `sa:${a.id}`, label: a.name, description: t("limits.serviceAccount", "Service account") })),
  ].filter(free);
  const keyOptions = (keys.data ?? [])
    .filter((k) => k.state !== "archived")
    .map((k) => ({ value: k.id, label: k.name, description: k.redacted }))
    .filter(free);

  const target: LimitTarget | null =
    level === "project"
      ? { level: "project" }
      : !subject
        ? null
        : level === "member"
          ? subject.startsWith("sa:")
            ? { level: "member", service_account: subject.slice(3) }
            : { level: "member", user: subject.slice(5) }
          : { level: "key", key: subject };
  const key = level === "key" ? keys.data?.find((k) => k.id === subject) : undefined;
  const owner: LimitOwner | undefined = key
    ? { kind: key.principal.kind === "service_account" ? "service_account" : "user", id: key.principal.id ?? "" }
    : undefined;
  const parents = useParentLimits(org, project, { level }, owner);
  const currency = useOrgCurrency(org);

  const add = useApiMutation({
    mutationFn: async (to: LimitTarget) =>
      data(
        await api.POST("/platform/v1/orgs/{org}/projects/{project}/limits", {
          params: { path: { org, project } },
          body: { ...to, ...limitBody(draft) },
        }),
      ),
    invalidates: [["orgs", org, "projects", project, "limits"]],
    onSuccess: (l) => {
      toast.success(t("limits.added", "Added the {limit}", { limit: limitDescription(l) }));
      onClose();
    },
  });
  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={copy ? t("limits.copyTitle", "Copy limit") : t("limits.add", "Add limit")}
      submitLabel={t("limits.add", "Add limit")}
      submitting={add.isPending}
      onSubmit={() => target && !limitBlocker(draft, parents, currency) && add.mutate(target)}
      size="md"
      steps={[
        {
          id: "who",
          title: t("limits.whoTitle", "Who does it apply to?"),
          description: t("limits.whoHint", "A request passes its key's, its member's and the project's limits."),
          blockedReason: target
            ? undefined
            : level === "member"
              ? t("limits.chooseMemberOrAccount", "Choose a member or service account")
              : t("limits.chooseKey", "Choose a key"),
          content: (
            <div className="flex flex-col gap-12">
              {!copy && (
                <Segmented
                  aria-label={t("limits.appliesTo", "Applies to")}
                  value={level}
                  onValueChange={(v) => {
                    setLevel(v as Level);
                    setSubject(null);
                  }}
                  options={LEVEL_IDS.map((l) => ({
                    value: l,
                    label: l === "member" ? t("limits.member", "Member") : l === "key" ? t("limits.key", "Key") : common.project(),
                  }))}
                  fullWidth
                />
              )}
              {level !== "project" && (
                <Combobox
                  aria-label={level === "member" ? t("limits.memberOrAccount", "Member or service account") : t("limits.key", "Key")}
                  placeholder={
                    level === "member" ? t("limits.chooseMemberOrAccount", "Choose a member or service account") : t("limits.chooseKey", "Choose a key")
                  }
                  options={level === "member" ? memberOptions : keyOptions}
                  value={subject}
                  onValueChange={(v: string | null) => setSubject(v)}
                />
              )}
            </div>
          ),
        },
        {
          id: "limit",
          title: t("limits.whatTitle", "What's the limit?"),
          description: t("limits.whatHint", "Rates refuse requests above them; budgets refuse or alert once used up."),
          blockedReason: limitBlocker(draft, parents, currency) ?? undefined,
          content: <LimitEditor org={org} value={draft} onChange={setDraft} models={catalogue.data ?? []} />,
        },
      ]}
    />
  );
}
