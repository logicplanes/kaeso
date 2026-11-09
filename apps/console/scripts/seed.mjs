import Anthropic from "@anthropic-ai/sdk";
import OpenAI, { toFile } from "openai";
import { DRAFT_SYSTEM, SLACK_SYSTEM, TRIAGE_SYSTEM, slackQuestion, ticket } from "../stack/conversations.mjs";
import { pictureUpload } from "./png.mjs";
import { clickhouse, writeHistory } from "./seed-history.mjs";
import { STACKS, psql, setupToken, urls } from "./stack.mjs";
import { sendTraffic } from "./traffic.mjs";

const stack = STACKS.dev;
const { gateway: GATEWAY, mail: MAIL, clickhouse: CLICKHOUSE } = urls(stack);
const CLICKHOUSE_URL = `http://kaeso:kaeso@${new URL(CLICKHOUSE).host}/?database=kaeso`;
const PASSWORD = "correct horse battery staple";
const OWNER = { name: "Ada Okafor", email: "owner@example.com", org: "Northwind Labs" };
const PEOPLE = [
  { name: "Lin Park", email: "lin@example.com", role: "developer" },
  { name: "Noor Haddad", email: "noor@example.com", role: "admin" },
  { name: "Mateo Silva", email: "mateo@example.com", role: "developer" },
  { name: "Priya Raman", email: "priya@example.com", role: "member" },
  { name: "Jonas Weber", email: "jonas@example.com", role: "member" },
];
const PROJECTS = [
  {
    name: "Checkout assistant",
    share: 5,
    openai: ["gpt-5.4-mini", "gpt-6-sol", "text-embedding-3-small", "gpt-image-2", "tts-1", "whisper-1"],
    anthropic: ["claude-haiku-5-5", "claude-sonnet-5-5"],
  },
  { name: "Support triage", share: 3, openai: ["gpt-5.4-mini", "gpt-5.4-nano"], anthropic: ["claude-haiku-5-5", "claude-sonnet-5-5", "claude-haiku-4-5"] },
  { name: "Search ranking", share: 2, openai: ["text-embedding-3-large", "text-embedding-3-small", "gpt-5.4-nano"], anthropic: [] },
  { name: "Research agent", share: 1.5, openai: ["gpt-6-sol", "gpt-5.6"], anthropic: ["claude-opus-5-5", "claude-haiku-5-5"] },
  { name: "Internal tools", share: 0.6, openai: ["gpt-5.4-mini", "gpt-image-1-mini"], anthropic: ["claude-haiku-5-5"] },
];
const TICKETS = 5000;
const QUESTIONS = 600;
const REQUEST_RATE = 60;
const CONTENT_DAYS = 30;

class Session {
  constructor(name) {
    this.name = name;
    this.cookie = "";
  }

  async call(method, path, body) {
    const res = await fetch(`${GATEWAY}/platform/v1/${path}`, {
      method,
      headers: {
        ...(body === undefined ? {} : { "content-type": "application/json" }),
        ...(method === "GET" ? {} : { origin: stack.publicUrl }),
        ...(this.cookie ? { cookie: this.cookie } : {}),
      },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    });
    for (const cookie of res.headers.getSetCookie()) {
      const pair = cookie.split(";")[0];
      if (pair.startsWith("kaeso_session")) this.cookie = pair;
    }
    const text = await res.text();
    if (!res.ok) throw new Error(`${this.name}: ${method} ${path} answered ${res.status} ${text}`);
    return text ? JSON.parse(text) : null;
  }

  get = (path) => this.call("GET", path);
  post = (path, body = {}) => this.call("POST", path, body);
  patch = (path, body) => this.call("PATCH", path, body);
  put = (path, body) => this.call("PUT", path, body);
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function mailpit(path) {
  const res = await fetch(`${MAIL}/api/v1/${path}`);
  if (!res.ok) throw new Error(`seed: Mailpit answered ${res.status} to ${path}: ${await res.text()}`);
  return res.json();
}

async function mailToken(to, kind) {
  for (let attempt = 0; attempt < 50; attempt++) {
    const list = await mailpit(`search?query=${encodeURIComponent(`to:${to}`)}`);
    for (const message of list.messages ?? []) {
      const full = await mailpit(`message/${message.ID}`);
      const match = new RegExp(`${kind}#token=([A-Za-z0-9_-]+)`).exec(full.Text ?? "");
      if (match) return match[1];
    }
    await sleep(200);
  }
  throw new Error(`seed: no ${kind} email for ${to}`);
}

async function signIn(email) {
  const session = new Session(email);
  await session.post("auth/password", { email, password: PASSWORD });
  return session;
}

async function addModels(owner, base, provider, wanted) {
  const ids = [];
  for (const query of wanted) {
    const hits = await owner.get(`${base}/models/search?provider=${provider}&q=${encodeURIComponent(query)}`);
    const hit = hits.data.find((h) => h.model_id === query);
    if (hit) ids.push(hit.model_id);
    // The search leaves out deprecated models, such as OpenAI's speech and transcription models,
    // which a project can still add by id.
    else await owner.post(`${base}/models`, { provider, model_id: query });
  }
  if (ids.length) await owner.post(`${base}/models/batch`, { provider, model_ids: ids });
}

async function invite(owner, org, person, index, project_roles) {
  await owner.post(`orgs/${org}/invites`, { email: person.email, role: person.role, project_roles });
  const token = await mailToken(person.email, "invite");
  await new Session(person.email).post("auth/invites/accept", { token, name: person.name, password: PASSWORD });
  const session = await signIn(person.email);
  await session.put("me/avatar", pictureUpload(index + 1));
  return { ...person, id: (await session.get("auth/session")).user.id, session };
}

async function buildOrg() {
  const token = setupToken(stack);
  if (!token) throw new Error("seed: this stack is already set up; run `pnpm stack:down && pnpm stack:up` for a fresh one");
  await fetch(`${MAIL}/api/v1/messages`, { method: "DELETE" });
  const owner = new Session("owner");
  await owner.post("auth/setup", { name: OWNER.name, email: OWNER.email, password: PASSWORD, org_name: OWNER.org, token });
  await owner.put("me/avatar", pictureUpload(0));
  const org = (await owner.get("auth/session")).memberships[0].org_id;
  await owner.put(`orgs/${org}/avatar`, pictureUpload(4));

  const projects = [];
  for (const project of PROJECTS) projects.push({ ...project, id: (await owner.post(`orgs/${org}/projects`, { name: project.name })).id });
  for (const [i, project] of projects.entries()) await owner.put(`orgs/${org}/projects/${project.id}/avatar`, pictureUpload(i + 2));

  await owner.post(`orgs/${org}/credentials`, { provider: "openai", name: "Production", secret: "sk-proj-mock-0000000000000000openai" });
  await owner.post(`orgs/${org}/credentials`, { provider: "anthropic", name: "Production", secret: "sk-ant-mock-00000000000000anthropic" });
  const supportKey = await owner.post(`orgs/${org}/credentials`, { provider: "openai", name: "Support team", secret: "sk-proj-mock-1111111111111support" });
  await owner.put(`orgs/${org}/credentials/${supportKey.id}/projects`, { projects: [projects[1].id] });

  for (const project of projects) {
    const base = `orgs/${org}/projects/${project.id}`;
    await addModels(owner, base, "openai", project.openai);
    await addModels(owner, base, "anthropic", project.anthropic);
    project.models = (await owner.get(`${base}/models`)).data;
  }
  const checkout = `orgs/${org}/projects/${projects[0].id}`;
  const seeded = (id) => projects[0].models.find((m) => m.model_id === id);
  const step = (model, fallback = null) => ({ type: "model", model: model.id, fallback });
  const [haiku, sonnet, mini, sol] = ["claude-haiku-5-5", "claude-sonnet-5-5", "gpt-5.4-mini", "gpt-6-sol"].map(seeded);
  if (haiku) await owner.post(`${checkout}/aliases`, { name: "fast", route: step(haiku) });
  if (sonnet && sol && mini)
    await owner.post(`${checkout}/aliases`, { name: "sturdy", route: step(sonnet, { ...step(sol, step(mini)), fall_back_on: ["unavailable", "limited"] }) });
  if (haiku && sonnet && mini && sol)
    await owner.post(`${checkout}/aliases`, {
      name: "support",
      route: {
        type: "condition",
        match: "any",
        rules: [
          { field: "header", name: "x-plan", op: "equals", value: "enterprise" },
          { field: "input_tokens", op: "at_least", value: 100000 },
        ],
        then: step(sonnet, step(sol)),
        else: {
          type: "split",
          by: "caller",
          branches: [
            { weight: 80, step: step(haiku, step(mini)) },
            { weight: 20, step: step(mini) },
          ],
        },
      },
    });
  if (haiku && sol)
    await owner.post(`${checkout}/aliases`, {
      name: "per-user",
      route: {
        type: "split",
        by: "header",
        header: "x-user-id",
        branches: [
          { weight: 50, step: step(haiku) },
          { weight: 50, step: step(sol) },
        ],
      },
    });

  const people = [];
  for (const [i, person] of PEOPLE.entries()) {
    const role = person.role === "member" ? "viewer" : person.role;
    people.push(await invite(owner, org, person, i, [{ project_id: projects[0].id, role }]));
  }
  await owner.post(`orgs/${org}/invites`, { email: "sam@example.com", role: "member" });
  const noor = people.find((p) => p.email === "noor@example.com");
  await owner.put(`instance/users/${noor.id}/instance-admin`, { is_instance_admin: true });
  for (const person of people.filter((p) => p.role !== "admin"))
    await owner.post(`orgs/${org}/projects/${projects[1].id}/members`, { user_id: person.id, role: person.role === "member" ? "viewer" : "developer" });

  const supportLead = await owner.post(`orgs/${org}/roles`, {
    scope: "project",
    name: "Support lead",
    description: "Calls models and reads request logs and usage to help customers",
    rights: ["project.view", "project.members.view", "api.request", "models.view", "logs.metadata.view", "logs.content.view", "usage.view", "costs.view"],
  });
  const worker = await owner.post(`${checkout}/service-accounts`, { name: "checkout-worker", role: supportLead.id });
  const ci = await owner.post(`${checkout}/service-accounts`, { name: "ci-runner", role: "developer" });

  const keys = [];
  const addKey = async (project, path, name, by = owner) => {
    const created = await by.post(`orgs/${org}/projects/${project.id}/${path}`, { name, expires_in_days: 90 });
    keys.push({ project, name, secret: created.secret });
  };
  await addKey(projects[0], "keys", "checkout-web");
  await addKey(projects[0], `service-accounts/${worker.id}/keys`, "checkout-worker");
  await addKey(projects[0], `service-accounts/${ci.id}/keys`, "ci-smoke-tests");
  await addKey(projects[1], "keys", "triage-bot");
  await addKey(projects[2], "keys", "ranking-service");
  await addKey(projects[3], "keys", "research-notebook");
  await addKey(projects[4], "keys", "slack-helper");
  const staging = await owner.post(`${checkout}/keys`, { name: "staging", expires_in_days: 30 });
  await owner.post(`${checkout}/keys/${staging.id}/disable`);
  for (const [email, name] of [
    ["lin@example.com", "lin-claude-code"],
    ["mateo@example.com", "mateo-codex"],
  ]) {
    const person = people.find((p) => p.email === email);
    await owner.post(`orgs/${org}/projects/${projects[3].id}/members`, { user_id: person.id, role: "developer" });
    await addKey(projects[3], "keys", name, person.session);
  }

  // Customer conversations, search queries and research stay out of the request log; support and
  // internal tools keep their content, and support answers repeated emails from the response cache.
  for (const project of [projects[0], projects[2], projects[3]])
    await owner.patch(`orgs/${org}/projects/${project.id}/logs/settings`, { content_logging: false });
  await owner.patch(`orgs/${org}/response-cache`, { allowed: true });
  await owner.patch(`orgs/${org}/projects/${projects[1].id}/response-cache`, { enabled: true, ttl_seconds: 3600 });

  const addLimits = async ({ budget, tokens }) => {
    await owner.post(`${checkout}/limits`, { level: "project", kind: "spend_budget", amount: String(budget), period: "month" });
    await owner.post(`${checkout}/limits`, { level: "project", kind: "request_rate", amount: String(REQUEST_RATE), period: "minute" });
    await owner.post(`${checkout}/limits`, {
      level: "member",
      service_account: worker.id,
      kind: "token_budget",
      amount: String(tokens),
      period: "day",
      hard: false,
    });
  };

  return { org, projects, keys, people, addLimits };
}

function clients(key) {
  return {
    openai: new OpenAI({ apiKey: key.secret, baseURL: `${GATEWAY}/v1`, maxRetries: 0 }),
    anthropic: new Anthropic({ apiKey: key.secret, baseURL: GATEWAY, maxRetries: 0 }),
  };
}

const cachedSystem = (text) => [{ type: "text", text, cache_control: { type: "ephemeral" } }];

// Every support email and Slack question once through the gateway, so the request log has their
// real content; the mock provider answers each with its written reply.
function conversations(keys) {
  const triage = clients(keys.find((k) => k.name === "triage-bot")).anthropic;
  const slack = clients(keys.find((k) => k.name === "slack-helper")).anthropic;
  const jobs = [];
  for (let i = 0; i < TICKETS; i++) {
    const t = ticket(i);
    jobs.push(() =>
      triage.messages.create({
        model: "claude-haiku-5-5",
        max_tokens: 400,
        system: cachedSystem(TRIAGE_SYSTEM),
        messages: [{ role: "user", content: t.classify.user }],
      }),
    );
    if (t.draftable)
      jobs.push(() =>
        triage.messages.create({
          model: "claude-sonnet-5-5",
          max_tokens: 600,
          system: cachedSystem(DRAFT_SYSTEM),
          messages: [{ role: "user", content: t.draft.user }],
        }),
      );
  }
  const asked = new Set();
  for (let i = 0; i < QUESTIONS; i++) {
    const q = slackQuestion(i);
    if (asked.has(q.user)) continue;
    asked.add(q.user);
    jobs.push(async () => {
      const stream = await slack.messages.create({
        model: "claude-haiku-5-5",
        max_tokens: 800,
        stream: true,
        system: SLACK_SYSTEM,
        messages: [{ role: "user", content: q.user }],
      });
      for await (const _ of stream);
    });
  }
  return jobs;
}

// How the checkout assistant's aliases route this key, and what the gateway records when a step
// fails over: one request per route, the failing ones with the mock's failure marker.
async function routes(keys) {
  const { openai } = clients(keys.find((k) => k.name === "checkout-web"));
  const ask = async (model, text, headers = {}) =>
    (await openai.chat.completions.create({ model, messages: [{ role: "user", content: `${text} [mock:tps=4000]` }] }, { headers })).model;
  const supportBranch = await ask("support", "Where is my parcel?");
  await ask("sturdy", "Where is my parcel? [mock:fail=claude-sonnet-5-5 overloaded]");
  await ask("support", "Where is my parcel? [mock:fail=claude-sonnet-5-5 overloaded]", { "x-plan": "enterprise" });
  if (supportBranch === "claude-haiku-5-5") await ask("support", "Where is my parcel? [mock:fail=claude-haiku-5-5 overloaded]");
  return { supportBranch, sent: 3 + (supportBranch === "claude-haiku-5-5" ? 1 : 0) };
}

// The gateway writes request logs and the usage rollup in batches of its own; both must be in before
// the seed clears them, or late rows would count the seed's own traffic.
async function flushed(count) {
  for (let attempt = 0; attempt < 240; attempt++) {
    const logged = Number(await clickhouse(CLICKHOUSE_URL, "SELECT count() FROM request_log"));
    const counted = Number(table("SELECT 'n', coalesce(sum(requests), 0) FROM usage_daily;")[0]?.[1] ?? 0);
    if (logged >= count && counted >= count) return;
    await sleep(500);
  }
  throw new Error("seed: the gateway didn't write its request logs and usage");
}

const rows = async (query) =>
  (await clickhouse(CLICKHOUSE_URL, `${query} FORMAT JSONEachRow`))
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));

function table(sql) {
  return psql(stack, `\\pset format unaligned\n\\pset tuples_only on\n${sql}`)
    .split("\n")
    .filter((line) => line.includes("|"))
    .map((line) => line.split("|"));
}

async function failovers() {
  const recorded = await rows("SELECT alias, fallbacks FROM request_log WHERE fallbacks != '' ORDER BY started_at");
  const steps = (alias, model) => recorded.find((r) => r.alias === alias && r.fallbacks.includes(`"anthropic/${model}"`));
  const as = (row) => {
    const parsed = JSON.parse(row.fallbacks);
    return (ms) => JSON.stringify(parsed.map((step, i) => (i === parsed.length - 1 ? { ...step, duration_ms: ms } : step)));
  };
  const found = {
    sturdy: steps("sturdy", "claude-sonnet-5-5"),
    "support-then": steps("support", "claude-sonnet-5-5"),
    "support-else": steps("support", "claude-haiku-5-5"),
  };
  return Object.fromEntries(
    Object.entries(found)
      .filter(([, row]) => row)
      .map(([name, row]) => [name, as(row)]),
  );
}

async function exemplars() {
  const logged = await rows(
    "SELECT l.input_tokens AS input, l.cache_read_tokens AS cacheRead, l.output_tokens AS output, c.request_body, c.reply FROM request_log AS l INNER JOIN request_content AS c ON l.request_id = c.request_id WHERE l.status = 200",
  );
  const byPrompt = new Map();
  for (const row of logged) {
    const body = JSON.parse(row.request_body);
    const last = body.messages?.at(-1)?.content;
    if (typeof last !== "string") continue;
    byPrompt.set(last, {
      request_body: row.request_body,
      reply: row.reply,
      replyId: JSON.parse(row.reply).id,
      shape: { input: row.input, cacheRead: row.cacheRead, output: row.output },
    });
  }
  const tickets = [];
  for (let i = 0; i < TICKETS; i++) {
    const t = ticket(i);
    const classify = byPrompt.get(t.classify.user);
    if (classify) tickets.push({ classify, draft: t.draftable ? (byPrompt.get(t.draft.user) ?? null) : null });
  }
  const slack = [...new Set(Array.from({ length: QUESTIONS }, (_, i) => slackQuestion(i).user))].map((q) => byPrompt.get(q)).filter(Boolean);
  if (tickets.length < TICKETS * 0.99 || !slack.length)
    throw new Error(`seed: only ${tickets.length} tickets and ${slack.length} questions came back with content`);
  return { tickets, slack };
}

function priceOf(projects) {
  const prices = new Map();
  for (const model of projects.flatMap((p) => p.models)) {
    const effective = model.effective ?? {};
    const input = Number(effective.input_price ?? 0);
    prices.set(model.model_id, {
      input,
      cached: Number(effective.cache_read_price ?? input),
      write: Number(effective.cache_write_price ?? input),
      output: Number(effective.output_price ?? 0),
    });
  }
  return (id) => prices.get(id) ?? { input: 0, cached: 0, write: 0, output: 0 };
}

// The organization was set up months ago, not just now: everything it holds, and the audit log
// that tells how, moves back to when it would have happened. Days ago for each part:
const AGE = {
  org: 160,
  people: { "noor@example.com": 145, "lin@example.com": 140, "mateo@example.com": 90, "priya@example.com": 60, "jonas@example.com": 35 },
  projects: { "Checkout assistant": 150, "Support triage": 120, "Search ranking": 100, "Research agent": 47, "Internal tools": 80 },
  credentials: { Production: 150, "Support team": 118 },
  accounts: { "checkout-worker": 148, "ci-runner": 130 },
  keys: {
    "checkout-web": 149,
    "checkout-worker": 147,
    "ci-smoke-tests": 129,
    "triage-bot": 119,
    "ranking-service": 99,
    "research-notebook": 46,
    "slack-helper": 79,
    staging: 31,
    "lin-claude-code": 44,
    "mateo-codex": 40,
  },
  aliases: { fast: 88, sturdy: 86, support: 74, "per-user": 72 },
  role: 125,
  limits: 40,
  settings: 31,
};

function backdate(people, projects) {
  const at = (days, minutes = 0) => `now() - interval '${days} days' + interval '${minutes} minutes'`;
  const joins = people.map((p) => [p.email, AGE.people[p.email]]).filter(([, days]) => days);
  const values = (pairs) => pairs.map(([name, days]) => `('${name}', ${days})`).join(", ");
  const timeline = [
    ["instance.setup", [AGE.org]],
    ["org.create", [AGE.org]],
    ["org.update", [AGE.org]],
    ["user.update", [AGE.org, ...joins.map(([, days]) => days)]],
    ["catalogue_model.create", projects.flatMap((p) => p.models.map(() => AGE.projects[p.name]))],
    ["project.create", Object.values(AGE.projects)],
    ["project.update", Object.values(AGE.projects)],
    ["credential.create", [AGE.credentials.Production, AGE.credentials.Production, AGE.credentials["Support team"]]],
    ["credential.assign", [AGE.credentials["Support team"]]],
    ["role.create", [AGE.role]],
    ["service_account.create", Object.values(AGE.accounts)],
    ["key.create", Object.values(AGE.keys)],
    ["key.disable", [21]],
    ["alias.create", Object.values(AGE.aliases)],
    ["limit.create", [AGE.limits, AGE.limits, AGE.limits - 2]],
    ["invite.create", joins.map(([, days]) => days + 1)],
    ["invite.accept", joins.map(([, days]) => days)],
    ["user.create_by_invite", joins.map(([, days]) => days)],
    ["user.instance_admin", [AGE.people["noor@example.com"]]],
    [
      "project.member_add",
      [
        ...joins.filter(([email]) => email !== "noor@example.com").map(([, days]) => Math.min(days, AGE.projects["Support triage"])),
        AGE.keys["lin-claude-code"],
        AGE.keys["mateo-codex"],
      ],
    ],
  ];
  const events = timeline.flatMap(([action, days]) => days.map((d, i) => `('${action}', ${i + 1}, ${d})`)).join(",\n    ");
  return `BEGIN;
SET LOCAL session_replication_role = replica;
UPDATE organizations SET created_at = ${at(AGE.org)}, updated_at = ${at(AGE.org, 90)};
UPDATE instance_settings SET created_at = ${at(AGE.org)}, setup_completed_at = ${at(AGE.org)};
UPDATE users SET created_at = ${at(AGE.org)}, updated_at = ${at(AGE.org, 30)}, email_verified_at = ${at(AGE.org)} WHERE email = '${OWNER.email}';
UPDATE users u SET created_at = ${at(0)} - make_interval(days => v.days), updated_at = ${at(0)} - make_interval(days => v.days) + interval '5 minutes',
  email_verified_at = ${at(0)} - make_interval(days => v.days)
  FROM (VALUES ${values(joins)}) AS v(email, days) WHERE u.email = v.email;
UPDATE org_memberships m SET created_at = u.created_at, updated_at = u.created_at FROM users u WHERE u.id = m.user_id;
UPDATE projects p SET created_at = ${at(0)} - make_interval(days => v.days), updated_at = ${at(0)} - make_interval(days => v.days) + interval '10 minutes'
  FROM (VALUES ${values(Object.entries(AGE.projects))}) AS v(name, days) WHERE p.name = v.name;
UPDATE project_memberships m SET created_at = greatest(p.created_at, u.created_at), updated_at = greatest(p.created_at, u.created_at)
  FROM projects p, users u WHERE p.id = m.project_id AND u.id = m.user_id;
UPDATE catalogue_models c SET created_at = p.created_at + interval '20 minutes', updated_at = p.created_at + interval '20 minutes' FROM projects p WHERE p.id = c.project_id;
UPDATE provider_credentials c SET created_at = ${at(0)} - make_interval(days => v.days), updated_at = ${at(0)} - make_interval(days => v.days)
  FROM (VALUES ${values(Object.entries(AGE.credentials))}) AS v(name, days) WHERE c.name = v.name;
UPDATE project_provider_keys k SET created_at = c.created_at + interval '2 minutes' FROM provider_credentials c WHERE c.id = k.credential_id;
UPDATE service_accounts a SET created_at = ${at(0)} - make_interval(days => v.days), updated_at = ${at(0)} - make_interval(days => v.days)
  FROM (VALUES ${values(Object.entries(AGE.accounts))}) AS v(name, days) WHERE a.name = v.name;
UPDATE api_keys k SET created_at = ${at(0)} - make_interval(days => v.days), updated_at = ${at(0)} - make_interval(days => v.days),
  expires_at = ${at(0)} - make_interval(days => v.days) + interval '365 days'
  FROM (VALUES ${values(Object.entries(AGE.keys))}) AS v(name, days) WHERE k.name = v.name;
UPDATE api_keys SET disabled_at = ${at(21)}, updated_at = ${at(21)} WHERE name = 'staging';
UPDATE aliases a SET created_at = ${at(0)} - make_interval(days => v.days), updated_at = ${at(0)} - make_interval(days => v.days)
  FROM (VALUES ${values(Object.entries(AGE.aliases))}) AS v(name, days) WHERE a.name = v.name;
UPDATE org_roles SET created_at = ${at(AGE.role)}, updated_at = ${at(AGE.role)} WHERE name = 'Support lead';
UPDATE limits SET created_at = ${at(AGE.limits)}, updated_at = ${at(AGE.limits)};
WITH ranked AS (
  SELECT id, action, row_number() OVER (PARTITION BY action ORDER BY at, id) AS n FROM audit_events
), story(action, n, days) AS (VALUES
    ${events}
)
UPDATE audit_events e SET at = ${at(0)} - make_interval(days => s.days) + make_interval(mins => r.n::int)
  FROM ranked r JOIN story s ON s.action = r.action AND s.n = r.n WHERE e.id = r.id;
UPDATE audit_events SET at = ${at(AGE.settings)} WHERE at > now() - interval '1 day' AND action NOT LIKE 'invite.%';
UPDATE invites SET created_at = now() - interval '4 days', updated_at = now() - interval '4 days', expires_at = now() + interval '17 days'
  WHERE accepted_at IS NULL AND revoked_at IS NULL;
UPDATE audit_events SET at = now() - interval '4 days' WHERE action = 'invite.create' AND at > now() - interval '1 day';
-- The audit log lists events by id, a UUIDv7 of when each was written; a moved event needs an id from its new time.
UPDATE audit_events SET id = uuidv7(at - clock_timestamp());
UPDATE audit_events SET ip = '198.51.100.7', user_agent = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/19.1 Safari/605.1.15'
  WHERE ip IS NOT NULL;
COMMIT;
`;
}

// The last time each key and provider key was used, from the history.
// A few media requests after the history, so the request log, usage and analytics show images,
// speech and transcripts: priced models in Checkout, whose spend budget refuses unpriced ones, and
// one image model without a price in Internal tools, whose cost shows as unknown.
async function media(keys) {
  const checkout = clients(keys.find((k) => k.name === "checkout-web")).openai;
  const internal = clients(keys.find((k) => k.name === "slack-helper")).openai;
  const audio = await toFile(
    Buffer.concat(Array.from({ length: 160 }, () => Buffer.concat([Buffer.from([0xff, 0xfb, 0x90, 0x00]), Buffer.alloc(413)]))),
    "call.mp3",
    {
      type: "audio/mpeg",
    },
  );
  await checkout.images.generate({ model: "gpt-image-2", prompt: "A parcel on a doorstep, flat illustration", size: "1024x1024" });
  await checkout.images.generate({ model: "gpt-image-2", prompt: "Two parcels, one opened", n: 2 });
  await (await checkout.audio.speech.create({ model: "tts-1", voice: "alloy", input: "Your parcel is on its way." })).arrayBuffer();
  await (
    await checkout.audio.speech.create({ model: "tts-1", voice: "nova", input: "We refunded your order; it reaches your account within three business days." })
  ).arrayBuffer();
  await checkout.audio.transcriptions.create({ model: "whisper-1", file: audio });
  await checkout.audio.transcriptions.create({ model: "whisper-1", file: audio, response_format: "text" });
  await internal.images.generate({ model: "gpt-image-1-mini", prompt: "A team photo of three robots" });
  return 7;
}

async function lastUsed() {
  const used = await rows("SELECT toString(key_id) AS key, toString(max(started_at)) AS at FROM request_log GROUP BY key_id");
  const last = await rows("SELECT toString(max(started_at)) AS at FROM request_log");
  if (!used.length) return "";
  return `UPDATE api_keys k SET last_used_at = v.at::timestamptz FROM (VALUES ${used.map((u) => `('${u.key}'::uuid, '${u.at}+00')`).join(", ")}) AS v(id, at) WHERE k.id = v.id;
UPDATE provider_credentials SET last_used_at = '${last[0].at}+00'::timestamptz;
UPDATE api_keys SET last_used_at = now() - interval '24 days' WHERE name = 'staging';
`;
}

// Limits a real team would set: the monthly budget a little above what Checkout spends, the
// worker's daily token budget just above a normal day so busy nights cross it.
async function limitsFor(keys) {
  const checkout = keys["checkout-web"].project_id;
  const [spend] = await rows(`SELECT sum(cost) AS cost FROM request_log WHERE project_id = toUUID('${checkout}') AND started_at >= now() - INTERVAL 30 DAY`);
  const [tokens] = await rows(
    `SELECT quantile(0.5)(t) AS t FROM (SELECT toDate(started_at) AS d, sum(input_tokens + output_tokens + cache_read_tokens) AS t FROM request_log WHERE key_id = toUUID('${keys["checkout-worker"].key_id}') GROUP BY d)`,
  );
  return {
    budget: Math.max(50, Math.ceil((Number(spend.cost) * 1.3) / 10) * 10),
    tokens: Math.max(100_000, Math.ceil((Number(tokens.t) * 1.15) / 100_000) * 100_000),
  };
}

const { org, projects, keys, people, addLimits } = await buildOrg();
const jobs = conversations(keys);
const { sent } = await sendTraffic(jobs, [], 8);
const route = await routes(keys);
await flushed(sent + route.sent);
const [fallbacks, examples] = [await failovers(), await exemplars()];
const keyRows = Object.fromEntries(
  table(
    "SELECT k.name, k.org_id, k.project_id, k.id, CASE WHEN k.user_id IS NULL THEN 'service_account' ELSE 'user' END, coalesce(k.user_id, k.service_account_id) FROM api_keys k;",
  ).map(([name, org_id, project_id, key_id, principal_kind, principal_id]) => [name, { org_id, project_id, key_id, principal_kind, principal_id }]),
);

// The conversations above were only sent to get their content; the history replaces them.
await clickhouse(CLICKHOUSE_URL, "TRUNCATE TABLE request_log");
await clickhouse(CLICKHOUSE_URL, "TRUNCATE TABLE request_content");
psql(stack, "DELETE FROM usage_daily;");

const now = Date.now();
const history = await writeHistory({
  clickhouseUrl: CLICKHOUSE_URL,
  days: 70,
  now,
  until: process.env.KAESO_SEED_WHOLE_DAYS ? Math.floor(now / 86_400_000) * 86_400_000 : undefined,
  keys: keyRows,
  prices: priceOf(projects),
  fallbacks,
  supportBranch: route.supportBranch,
  requestRate: REQUEST_RATE,
  exemplars: examples,
  contentDays: CONTENT_DAYS,
  recordUsage: (sql) => psql(stack, sql),
});
await addLimits(await limitsFor(keyRows));
const mediaSent = await media(keys);
psql(stack, backdate(people, projects) + (await lastUsed()));
console.log(`seed: ${OWNER.org} (${org}) with ${projects.length} projects, ${people.length + 1} members, ${keys.length + 1} keys`);
console.log(`seed: ${history.written} requests over 70 days, ${history.contents} of them with content (${sent} conversations sent through the gateway for it)`);
console.log(
  `seed: Anthropic overload ${history.outageDay} days ago, autumn sale ${history.saleDay} days ago, reindex rate limits ${history.reindexIncident} days ago`,
);
console.log(`seed: ${mediaSent} media requests: images, speech and transcripts`);
console.log(`seed: sign in at ${stack.publicUrl}/sign-in as ${OWNER.email} with the password "${PASSWORD}"`);
