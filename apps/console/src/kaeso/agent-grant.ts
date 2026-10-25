import type { AgentCatalogue, GrantBody, Grantable, GrantableOrg } from "~/api/generated/types";
import { locale, t } from "~/i18n";

export type PresetId = "observer" | "builder" | "operator" | "full";
export const DEFAULT_PRESET: PresetId = "builder";

export interface OrgDraft {
  included: boolean;
  rights: string[];
  all: boolean;
  projectRights: string[];
  selected: Record<string, string[]>;
}

export interface AgentDraft {
  preset: PresetId;
  customized: boolean;
  instance: string[];
  account: string[];
  orgs: Record<string, OrgDraft>;
  future: { on: boolean; rights: string[]; projectRights: string[] };
}

const ACCOUNT_VIEW = "account.view";

const within = (rights: readonly string[], allowed: readonly string[]): string[] => rights.filter((r) => allowed.includes(r));

const asPreset = (id: string): PresetId => (["observer", "builder", "operator", "full"].includes(id) ? (id as PresetId) : DEFAULT_PRESET);

export function presetPart(cat: AgentCatalogue, id: string, part: "org" | "project" | "account"): string[] {
  return cat.presets.filter((p) => p.id === id).flatMap((p) => p[part]);
}

export function heldInProjects(org: GrantableOrg): string[] {
  const held = new Set(org.every_project);
  for (const p of org.projects) for (const r of p.rights) held.add(r);
  return [...held];
}

export function orderOf(cat: AgentCatalogue, scope: "instance" | "account" | "org" | "project"): string[] {
  return cat[scope].categories.flatMap((c) => c.rights.map((r) => r.id));
}

export function sorted(cat: AgentCatalogue, scope: "instance" | "account" | "org" | "project", rights: Iterable<string>): string[] {
  const set = new Set(rights);
  return orderOf(cat, scope).filter((r) => set.has(r));
}

export function presetDraft(cat: AgentCatalogue, g: Grantable, id: PresetId): AgentDraft {
  const instance = cat.presets.filter((p) => p.id === id).map((p) => p.instance);
  const orgs: Record<string, OrgDraft> = {};
  for (const o of g.orgs) {
    orgs[o.id] = {
      included: o.allowed,
      rights: within(presetPart(cat, id, "org"), o.rights),
      all: true,
      projectRights: within(presetPart(cat, id, "project"), heldInProjects(o)),
      selected: {},
    };
  }
  return {
    preset: id,
    customized: false,
    instance: within(
      cat.instance_presets.filter((i) => instance.includes(i.id)).flatMap((i) => i.rights),
      g.instance,
    ),
    account: [...new Set([ACCOUNT_VIEW, ...within(presetPart(cat, id, "account"), g.account)])],
    orgs,
    future: { on: true, rights: presetPart(cat, id, "org"), projectRights: presetPart(cat, id, "project") },
  };
}

export function fromGrant(body: GrantBody, g: Grantable): AgentDraft {
  const orgs: Record<string, OrgDraft> = {};
  const future = body.future_orgs;
  for (const o of g.orgs)
    orgs[o.id] =
      future && o.allowed
        ? {
            included: true,
            rights: within(future.rights ?? [], o.rights),
            all: true,
            projectRights: within(future.project_rights ?? [], heldInProjects(o)),
            selected: {},
          }
        : { included: false, rights: [], all: true, projectRights: [], selected: {} };
  for (const o of body.orgs ?? []) {
    orgs[o.org] = {
      included: o.included,
      rights: o.rights ?? [],
      all: o.projects?.all ?? true,
      projectRights: o.projects?.rights ?? [],
      selected: Object.fromEntries((o.projects?.selected ?? []).map((p) => [p.project, p.rights])),
    };
  }
  return {
    preset: asPreset(body.preset),
    customized: !!body.customized,
    instance: body.instance ?? [],
    account: [...new Set([ACCOUNT_VIEW, ...(body.account ?? [])])],
    orgs,
    future: future ? { on: true, rights: future.rights ?? [], projectRights: future.project_rights ?? [] } : { on: false, rights: [], projectRights: [] },
  };
}

export function toGrant(d: AgentDraft): GrantBody {
  return {
    preset: d.preset,
    customized: d.customized,
    instance: d.instance,
    account: d.account,
    orgs: Object.entries(d.orgs).map(([org, o]) => ({
      org,
      included: o.included,
      rights: o.included ? o.rights : [],
      projects: o.all
        ? { all: true, rights: o.included ? o.projectRights : [], selected: [] }
        : { all: false, rights: [], selected: o.included ? Object.entries(o.selected).map(([project, rights]) => ({ project, rights })) : [] },
    })),
    future_orgs: d.future.on ? { rights: d.future.rights, project_rights: d.future.projectRights } : null,
  };
}

function rightsOf(d: AgentDraft): Map<string, string> {
  const out = new Map<string, string>();
  for (const r of d.instance) out.set(`instance:${r}`, r);
  for (const r of d.account) out.set(`account:${r}`, r);
  for (const [org, o] of Object.entries(d.orgs)) {
    if (!o.included) continue;
    for (const r of o.rights) out.set(`${org}:${r}`, r);
    if (o.all) for (const r of o.projectRights) out.set(`${org}:*:${r}`, r);
    else for (const [p, rights] of Object.entries(o.selected)) for (const r of rights) out.set(`${org}:${p}:${r}`, r);
  }
  if (d.future.on) {
    for (const r of d.future.rights) out.set(`future:${r}`, r);
    for (const r of d.future.projectRights) out.set(`future:*:${r}`, r);
  }
  return out;
}

export function gainedSensitive(cat: AgentCatalogue, before: AgentDraft | null, after: AgentDraft): string[] {
  const sensitive = new Set(
    (["instance", "account", "org", "project"] as const).flatMap((s) => cat[s].categories.flatMap((c) => c.rights.filter((r) => r.sensitive).map((r) => r.id))),
  );
  const had = before ? rightsOf(before) : new Map<string, string>();
  const gained = new Set<string>();
  for (const [where, right] of rightsOf(after)) {
    if (!sensitive.has(right) || had.has(where)) continue;
    const every = where.replace(/:[^:]+:([^:]+)$/, ":*:$1");
    if (had.has(every)) continue;
    gained.add(right);
  }
  return [...gained];
}

type Topic = "settings" | "members" | "projects" | "providers" | "security" | "keys" | "logs" | "usage" | "audit" | "models" | "limits";

const TOPIC_OF: Readonly<Record<string, Topic>> = {
  organization: "settings",
  members: "members",
  projects: "projects",
  providers: "providers",
  security: "security",
  keys: "keys",
  logs: "logs",
  usage: "usage",
  audit: "audit",
  project: "projects",
  model_use: "models",
  models: "models",
  limits: "limits",
};

const TOPICS: Readonly<Record<Topic, () => string>> = {
  settings: () => t("kaeso.agents.topic.settings", "settings"),
  members: () => t("kaeso.agents.topic.members", "members"),
  projects: () => t("kaeso.agents.topic.projects", "projects"),
  providers: () => t("kaeso.agents.topic.providers", "providers"),
  security: () => t("kaeso.agents.topic.security", "security"),
  keys: () => t("kaeso.agents.topic.keys", "keys"),
  logs: () => t("kaeso.agents.topic.logs", "logs"),
  usage: () => t("kaeso.agents.topic.usage", "usage and costs"),
  audit: () => t("kaeso.agents.topic.audit", "audit logs"),
  models: () => t("kaeso.agents.topic.models", "models"),
  limits: () => t("kaeso.agents.topic.limits", "limits"),
};

const changes = (right: string) => !/\.view(\.|$)/.test(right) && right !== "api.request";

export const listOf = (items: string[]): string => new Intl.ListFormat(locale(), { type: "conjunction" }).format(items);

function scopeWords(cat: AgentCatalogue, g: Grantable, d: AgentDraft) {
  const categoryOf = new Map<string, string>();
  for (const scope of ["org", "project"] as const) for (const c of cat[scope].categories) for (const r of c.rights) categoryOf.set(r.id, c.id);
  const managed = new Set<Topic>();
  const read = new Set<Topic>();
  const orgNames: string[] = [];
  for (const o of g.orgs) {
    const draft = d.orgs[o.id];
    if (!draft?.included) continue;
    orgNames.push(o.name);
    const rights = [...draft.rights, ...(draft.all ? draft.projectRights : Object.values(draft.selected).flat())];
    for (const r of rights) {
      const topic = TOPIC_OF[categoryOf.get(r) ?? ""];
      if (!topic) continue;
      (changes(r) ? managed : read).add(topic);
    }
  }
  for (const topic of managed) read.delete(topic);
  const order = Object.keys(TOPICS) as Topic[];
  const words = (has: (x: Topic) => boolean) => {
    const [first = "", second = "", ...rest] = order.filter(has).map((x) => TOPICS[x]());
    return listOf(rest.length > 1 ? [first, second, t("kaeso.agents.more", "{count} more", { count: rest.length })] : [first, second, ...rest].filter(Boolean));
  };
  const orgs = orgNames.length > 2 ? t("kaeso.agents.orgCount", "{count} organizations", { count: orgNames.length }) : listOf(orgNames);
  const acts = orgNames.length > 0 && managed.size + read.size > 0;
  return {
    managed: words((x) => managed.has(x)),
    read: words((x) => read.has(x)),
    unmanaged: words((x) => !managed.has(x)),
    unread: words((x) => !read.has(x) && !managed.has(x)),
    manages: managed.size > 0,
    reads: read.size > 0,
    managesAll: managed.size === order.length,
    managesMost: managed.size >= order.length - 3,
    readsMost: !managed.size && read.size >= order.length - 3,
    orgs,
    acts,
  };
}

export function summary(cat: AgentCatalogue, g: Grantable, d: AgentDraft): string {
  const w = scopeWords(cat, g, d);
  const instance = d.instance.length
    ? d.instance.some(changes)
      ? t("kaeso.agents.summaryInstanceManage", "Also helps run the instance.")
      : t("kaeso.agents.summaryInstanceRead", "Also reads the instance.")
    : "";
  const main = !w.acts
    ? d.future.on
      ? t("kaeso.agents.summaryFutureOnly", "Acts only in organizations you join later.")
      : t("kaeso.agents.summaryNothing", "Reads your profile, nothing in your organizations.")
    : t(
        "kaeso.agents.summary",
        "{kind, select, both {Manages {managed} and reads the rest in {orgs}.} manage {Manages {managed} in {orgs}.} other {Reads {read} in {orgs}.}}",
        { kind: w.manages && w.reads ? "both" : w.manages ? "manage" : "read", managed: w.managed, read: w.read, orgs: w.orgs },
      );
  return instance ? `${main} ${instance}` : main;
}

export function summaryLines(cat: AgentCatalogue, g: Grantable, d: AgentDraft): string[] {
  const w = scopeWords(cat, g, d);
  const lines: string[] = [];
  if (w.acts && w.manages)
    lines.push(
      w.managesAll
        ? t("kaeso.agents.lineManageAll", "Manages all of {orgs}", { orgs: w.orgs })
        : w.managesMost
          ? t("kaeso.agents.lineManageMost", "Manages all of {orgs} except {topics}", { orgs: w.orgs, topics: w.unmanaged })
          : t("kaeso.agents.lineManage", "Manages {topics} in {orgs}", { topics: w.managed, orgs: w.orgs }),
    );
  if (w.acts && w.reads)
    lines.push(
      w.manages
        ? t("kaeso.agents.lineReadRest", "Reads the rest of {orgs}", { orgs: w.orgs })
        : w.readsMost
          ? t("kaeso.agents.lineReadMost", "Reads all of {orgs} except {topics}", { orgs: w.orgs, topics: w.unread })
          : t("kaeso.agents.lineRead", "Reads {topics} in {orgs}", { topics: w.read, orgs: w.orgs }),
    );
  if (d.future.on) lines.push(t("kaeso.agents.lineFuture", "Also in organizations you join later"));
  if (d.instance.length)
    lines.push(
      d.instance.some(changes) ? t("kaeso.agents.lineInstanceManage", "Helps run the instance") : t("kaeso.agents.lineInstanceRead", "Reads the instance"),
    );
  if (!lines.length) lines.push(t("kaeso.agents.lineProfile", "Reads your profile, nothing else"));
  return lines;
}

function withNeeds(cat: AgentCatalogue, scope: "instance" | "account" | "org" | "project", rights: Iterable<string>): string[] {
  const needsOf = (right: string) => cat[scope].categories.flatMap((c) => c.rights.filter((r) => r.id === right).flatMap((r) => r.needs));
  const out = new Set<string>();
  const add = (right: string) => {
    if (out.has(right)) return;
    out.add(right);
    needsOf(right).forEach(add);
  };
  [...rights].forEach(add);
  return sorted(cat, scope, out);
}

export function rightLabels(cat: AgentCatalogue, ids: readonly string[]): string {
  return ids
    .map((id) =>
      (["instance", "account", "org", "project"] as const)
        .flatMap((s) => cat[s].categories.flatMap((c) => c.rights.filter((r) => r.id === id).map((r) => r.name)))
        .join(""),
    )
    .join(t("kaeso.agents.listSeparator", ", "));
}

export function scopeOf(cat: AgentCatalogue, right: string): "instance" | "account" | "org" | "project" | undefined {
  return (["instance", "account", "org", "project"] as const).find((s) => cat[s].categories.some((c) => c.rights.some((r) => r.id === right)));
}

export function withRequested(
  cat: AgentCatalogue,
  g: Grantable,
  d: AgentDraft,
  right: string,
  org?: string,
  project?: string,
): { draft: AgentDraft; held: boolean } {
  const unchanged = { draft: d, held: false };
  const scope = scopeOf(cat, right);
  if (scope === "instance" || scope === "account") {
    if (!g[scope].includes(right)) return unchanged;
    return { draft: { ...d, customized: true, [scope]: withNeeds(cat, scope, [...d[scope], right]) }, held: true };
  }
  const target = g.orgs.find((x) => x.id === org);
  const place = target?.projects.find((x) => x.id === project);
  const holds = scope === "org" ? target?.rights : place ? place.rights : target && heldInProjects(target);
  if (!scope || !target || !holds?.includes(right)) return unchanged;
  const current = d.orgs[target.id] ?? { included: false, rights: [], all: true, projectRights: [], selected: {} };
  const o: OrgDraft = current.included
    ? { ...current, selected: { ...current.selected } }
    : { included: true, rights: [], all: !place, projectRights: [], selected: {} };
  if (scope === "org") o.rights = withNeeds(cat, "org", [...o.rights, right]);
  else {
    if (o.all || !place) o.projectRights = withNeeds(cat, "project", [...o.projectRights, right]);
    else o.selected[place.id] = withNeeds(cat, "project", [...(o.selected[place.id] ?? []), right]);
    o.rights = withNeeds(cat, "org", o.rights.length ? o.rights : ["org.view"]);
  }
  return { draft: { ...d, customized: true, orgs: { ...d.orgs, [target.id]: o } }, held: true };
}
