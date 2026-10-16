import Anthropic from "@anthropic-ai/sdk";
import OpenAI from "openai";
import { pictureUpload } from "./png.mjs";
import { clickhouse, writeHistory } from "./seed-history.mjs";
import { STACKS, setupToken, urls } from "./stack.mjs";
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
    openai: ["gpt-5.4-mini", "gpt-6-sol", "text-embedding-3-small"],
    anthropic: ["claude-haiku-5-5", "claude-sonnet-5-5"],
  },
  { name: "Support triage", share: 3, openai: ["gpt-5.4-mini", "gpt-5.4-nano"], anthropic: ["claude-sonnet-5-5", "claude-haiku-4-5"] },
  { name: "Search ranking", share: 2, openai: ["text-embedding-3-large", "text-embedding-3-small", "gpt-5.4-nano"], anthropic: [] },
  { name: "Research agent", share: 1.5, openai: ["gpt-6-sol", "gpt-5.6"], anthropic: ["claude-opus-5-5"] },
  { name: "Internal tools", share: 0.6, openai: ["gpt-5.4-mini"], anthropic: ["claude-haiku-5-5"] },
];
const PROMPTS = [
  "Summarise the order status for 4471.",
  "Draft a polite reply about a late delivery.",
  "Classify this ticket: refund request for a damaged parcel.",
  "Which shipping option is cheapest for a 2 kg parcel to Lyon?",
  "Rewrite this product description in a friendlier tone.",
  "List the three most common reasons for returns this week.",
];

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
  }
  if (ids.length) await owner.post(`${base}/models/batch`, { provider, model_ids: ids });
}

async function invite(owner, org, person, index) {
  await owner.post(`orgs/${org}/invites`, { email: person.email, role: person.role });
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

  const first = (await owner.get("me/memberships")).data[0].projects.find((p) => p.is_default).id;
  await owner.patch(`orgs/${org}/projects/${first}`, { name: PROJECTS[0].name });
  const projects = [{ ...PROJECTS[0], id: first }];
  for (const project of PROJECTS.slice(1)) projects.push({ ...project, id: (await owner.post(`orgs/${org}/projects`, { name: project.name })).id });
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
  const haiku = projects[0].models.find((m) => m.model_id === "claude-haiku-5-5");
  if (haiku) await owner.post(`${checkout}/aliases`, { name: "fast", catalogue_model_id: haiku.id });

  const people = [];
  for (const [i, person] of PEOPLE.entries()) people.push(await invite(owner, org, person, i));
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
  const addKey = async (project, path, name, extra = {}) => {
    const created = await owner.post(`orgs/${org}/projects/${project.id}/${path}`, { name, expires_in_days: 90, ...extra });
    keys.push({ project, secret: created.secret });
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

  const addLimits = async () => {
    await owner.post(`${checkout}/limits`, { level: "project", kind: "spend_budget", amount: "1500", period: "month" });
    await owner.post(`${checkout}/limits`, { level: "project", kind: "request_rate", amount: "600", period: "minute" });
    await owner.post(`${checkout}/limits`, {
      level: "member",
      service_account: worker.id,
      kind: "token_budget",
      amount: "20000000",
      period: "day",
      hard: false,
    });
  };

  return { org, projects, keys, people, addLimits };
}

function requestsFor(key, project, index) {
  const prompt = PROMPTS[index % PROMPTS.length];
  const marker = "[mock:tps=4000]";
  const openai = new OpenAI({ apiKey: key.secret, baseURL: `${GATEWAY}/v1`, maxRetries: 0 });
  const anthropic = new Anthropic({ apiKey: key.secret, baseURL: GATEWAY, maxRetries: 0 });
  return project.models.map((model) => {
    if (model.model_id.startsWith("text-embedding")) return () => openai.embeddings.create({ model: model.model_id, input: [prompt, "shipping times"] });
    if (model.provider === "anthropic")
      return index % 3 === 0
        ? async () => {
            const stream = await anthropic.messages.create({
              model: model.model_id,
              max_tokens: 600,
              stream: true,
              messages: [{ role: "user", content: `${prompt} ${marker}` }],
            });
            for await (const _ of stream);
          }
        : () => anthropic.messages.create({ model: model.model_id, max_tokens: 600, messages: [{ role: "user", content: `${prompt} ${marker}` }] });
    if (index % 4 === 1)
      return () =>
        openai.chat.completions.create({
          model: model.model_id,
          messages: [{ role: "user", content: `Look up order 4471 for the customer. ${marker}` }],
          tools: [{ type: "function", function: { name: "lookup_order", parameters: { type: "object", properties: { order_id: { type: "string" } } } } }],
        });
    return index % 2 === 0
      ? () => openai.responses.create({ model: model.model_id, input: `${prompt} ${marker}` })
      : async () => {
          const stream = await openai.chat.completions.create({
            model: model.model_id,
            stream: true,
            messages: [{ role: "user", content: `${prompt} ${marker}` }],
          });
          for await (const _ of stream);
        };
  });
}

async function liveTraffic(keys) {
  const jobs = [];
  for (const key of keys) for (let round = 0; round < Math.round(key.project.share * 3); round++) jobs.push(...requestsFor(key, key.project, round));
  const failing = new OpenAI({ apiKey: keys[0].secret, baseURL: `${GATEWAY}/v1`, maxRetries: 0 });
  const refused = ["overloaded", "rate-limit", "server-error"].map(
    (marker) => () =>
      failing.chat.completions.create({ model: "gpt-5.4-mini", messages: [{ role: "user", content: `Where is order 4471? [mock:error=${marker}]` }] }),
  );
  return sendTraffic(jobs, refused);
}

async function flushed(count) {
  for (let attempt = 0; attempt < 60; attempt++) {
    if (Number(await clickhouse(CLICKHOUSE_URL, "SELECT count() FROM request_log")) >= count) return;
    await sleep(500);
  }
  throw new Error("seed: the gateway didn't write its request logs");
}

function priceOf(projects) {
  const prices = new Map();
  for (const model of projects.flatMap((p) => p.models)) {
    const input = Number(model.effective?.input_price ?? 0);
    prices.set(model.model_id, { input, cached: Number(model.effective?.cache_read_price ?? input), output: Number(model.effective?.output_price ?? 0) });
  }
  return (id) => prices.get(id) ?? { input: 0, cached: 0, output: 0 };
}

const { org, projects, keys, people, addLimits } = await buildOrg();
const { sent, refused } = await liveTraffic(keys);
await addLimits();
await flushed(sent);
const volume = new Map(
  (
    await clickhouse(
      CLICKHOUSE_URL,
      "SELECT toString(project_id), count(), uniqExact(key_id, resolved_model, endpoint) FROM request_log GROUP BY project_id FORMAT TSV",
    )
  )
    .trim()
    .split("\n")
    .map((line) => line.split("\t"))
    .map(([id, requests, kinds]) => [id, Number(requests) / Number(kinds)]),
);
const history = await writeHistory({
  clickhouseUrl: CLICKHOUSE_URL,
  days: 70,
  perDay: 2400,
  prices: priceOf(projects),
  weights: (template) => volume.get(template.project_id) ?? 1,
});
console.log(`seed: ${OWNER.org} (${org}) with ${projects.length} projects, ${people.length + 1} members, ${keys.length + 1} keys`);
console.log(`seed: ${sent} requests through the gateway (${refused} refused on purpose) and ${history} earlier requests over 70 days`);
console.log(`seed: sign in at ${stack.publicUrl}/sign-in as ${OWNER.email} with the password "${PASSWORD}"`);
