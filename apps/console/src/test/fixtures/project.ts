import type {
  AliasView,
  CatalogueModelView,
  GrantView,
  KeyView,
  LimitView,
  LogView,
  MemberUser,
  ModelAccess,
  Preview,
  ProjectLogSettings,
  ProjectMemberView,
  ProjectRightsView,
  SearchHit,
  ServiceAccountView,
  UsageRow,
} from "~/api/generated/types";
import { NOW, ORG, PROJECT, USER, roles } from "~/test/fixtures";
import { apiError, on, page } from "~/test/network";

const DAY_AGO = "2026-10-14T12:00:00Z";

export const listed = page;

export function grant(roleId = "developer", overrides: Partial<GrantView> = {}): GrantView {
  const role = roles.find((r) => r.scope === "project" && r.id === roleId) ?? roles.find((r) => r.scope === "project");
  return {
    role: { id: role?.id ?? roleId, kind: role?.kind ?? "builtin", name: role?.name ?? roleId },
    rights: role?.rights ?? [],
    rights_added: [],
    rights_removed: [],
    ...overrides,
  };
}

export function memberUser(overrides: Partial<MemberUser> = {}): MemberUser {
  return { id: "usr_grace", name: "Grace Hopper", email: "grace@example.com", state: "active", avatar_url: null, ...overrides };
}

export function projectMember(overrides: Partial<Omit<ProjectMemberView, "user">> & { user?: Partial<MemberUser> } = {}): ProjectMemberView {
  const { user, ...rest } = overrides;
  return { id: `pm_${user?.id ?? "usr_grace"}`, joined_at: NOW, grant: grant(), user: memberUser(user), ...rest };
}

export function me(overrides: Partial<Omit<ProjectMemberView, "user">> & { user?: Partial<MemberUser> } = {}): ProjectMemberView {
  const { user, ...rest } = overrides;
  return projectMember({ grant: grant("admin"), user: { id: USER, name: "Ada Okafor", email: "owner@example.com", ...user }, ...rest });
}

export function serviceAccount(overrides: Partial<ServiceAccountView> = {}): ServiceAccountView {
  return {
    id: "sa_deploy",
    name: "ci-deploy",
    project_id: PROJECT,
    created_at: NOW,
    created_by: USER,
    archived_at: null,
    grant: grant("developer"),
    ...overrides,
  };
}

export function apiKey(overrides: Partial<KeyView> = {}): KeyView {
  return {
    id: "key_web",
    name: "checkout-web",
    redacted: "sk-kso-AbC123…wxyz",
    state: "active",
    access: "inference",
    project_id: PROJECT,
    created_at: NOW,
    created_by: { kind: "user", id: USER },
    principal: { kind: "user", id: USER },
    expires_at: null,
    last_used_at: null,
    ...overrides,
  };
}

export function limit(overrides: Partial<LimitView> = {}): LimitView {
  return {
    id: "lim_month",
    level: "project",
    kind: "spend_budget",
    period: "month",
    amount: "50",
    used: "12.5",
    hard: true,
    alert_thresholds: [80],
    model: null,
    resets_at: "2026-11-01T00:00:00Z",
    subject: { type: "project", id: PROJECT },
    description: "monthly spend budget of 50 USD",
    revision: 1,
    created_at: NOW,
    updated_at: NOW,
    ...overrides,
  };
}

export function catalogueModel(overrides: Partial<CatalogueModelView> = {}): CatalogueModelView {
  return {
    id: "cm_sonnet",
    provider: "anthropic",
    model_id: "claude-sonnet-4-5",
    egress_dialect: "anthropic_messages",
    listed: true,
    custom: false,
    delisted_at: null,
    aliases: [],
    revision: 1,
    created_at: NOW,
    created_by: { kind: "user", id: USER },
    updated_at: NOW,
    overrides: {},
    effective: {
      display_name: "Claude Sonnet 4.5",
      input_modalities: ["text", "image"],
      output_modalities: ["text"],
      context_limit: 200000,
      output_limit: 64000,
      input_price: "3",
      output_price: "15",
      tool_call: true,
      reasoning: true,
      structured_output: false,
      temperature: true,
      release_date: "2025-09-29",
    },
    ...overrides,
  };
}

export function alias(overrides: Partial<AliasView> = {}): AliasView {
  return {
    id: "al_fast",
    name: "fast",
    catalogue_model_id: "cm_sonnet",
    target: "anthropic/claude-sonnet-4-5",
    shadows_model: false,
    created_at: NOW,
    updated_at: NOW,
    ...overrides,
  };
}

export function searchHit(overrides: Partial<SearchHit> = {}): SearchHit {
  return {
    provider: "openai",
    model_id: "gpt-5",
    name: "GPT-5",
    family: "gpt",
    deprecated: false,
    release_date: "2025-08-07",
    catalogue_model_id: null,
    ...overrides,
  };
}

export function preview(overrides: Partial<Preview> = {}): Preview {
  return {
    provider: "openai",
    model_id: "gpt-5",
    egress_dialect: "openai_responses",
    catalogue_model_id: null,
    fields: {
      display_name: "GPT-5",
      input_modalities: ["text", "image"],
      output_modalities: ["text"],
      context_limit: 400000,
      output_limit: 128000,
      input_price: "1.25",
      output_price: "10",
      tool_call: true,
      reasoning: true,
      structured_output: true,
      temperature: false,
    },
    ...overrides,
  };
}

export function usageRow(overrides: Partial<UsageRow> = {}): UsageRow {
  return {
    key: "2026-10-14",
    requests: 120,
    errors: 0,
    input_tokens: 1000,
    output_tokens: 500,
    cache_read_tokens: 0,
    cache_write_tokens: 0,
    reasoning_tokens: 0,
    cost: "1.25",
    ...overrides,
  };
}

export function failedLog(overrides: Partial<LogView> = {}): LogView {
  return {
    id: "log_1",
    status: 429,
    error_kind: "rate_limited",
    error_message: "Too many requests",
    requested_model: "claude-sonnet-4-5",
    model: "claude-sonnet-4-5",
    provider: "anthropic",
    started_at: DAY_AGO,
    client_ip: "127.0.0.1",
    cost: "0",
    duration_ms: 12,
    egress_dialect: "anthropic_messages",
    ingress_dialect: "anthropic_messages",
    endpoint: "/v1/messages",
    has_content: false,
    key_id: null,
    principal_id: USER,
    principal_kind: "user",
    source: "key",
    stream: false,
    upstream_request_id: "",
    user_agent: "test",
    usage: { input_tokens: 0, output_tokens: 0, cache_read_tokens: 0, cache_write_tokens: 0, reasoning_tokens: 0, estimated: false },
    ...overrides,
  };
}

export function access(overrides: Partial<ModelAccess> = {}): ModelAccess {
  return { all: true, models: [], effective: [], ...overrides };
}

export function memberRights(rights: string[], user = "usr_grace"): ProjectRightsView {
  return {
    project_id: PROJECT,
    user_id: user,
    role: { id: "developer", kind: "builtin", name: "Developer" },
    removed: [],
    rights: rights.map((right) => ({ right, sources: [{ kind: "role" }] })),
  } as ProjectRightsView;
}

export function logSettings(overrides: Partial<ProjectLogSettings> = {}): ProjectLogSettings {
  return { content_logging: true, effective_content_logging: true, ...overrides };
}

export const projectPath = (tail = "") => `/orgs/${ORG}/projects/${PROJECT}${tail ? `/${tail}` : ""}`;

export interface ProjectData {
  keys?: KeyView[];
  members?: ProjectMemberView[];
  accounts?: ServiceAccountView[];
  limits?: LimitView[];
  models?: CatalogueModelView[];
  aliases?: AliasView[];
}

export function projectData(d: ProjectData = {}): void {
  on("GET", "orgs/:org/projects/:project/keys", listed(d.keys ?? []));
  on("GET", "orgs/:org/projects/:project/members", listed(d.members ?? []));
  on("GET", "orgs/:org/projects/:project/service-accounts", ({ query }: { query: URLSearchParams }) =>
    listed((d.accounts ?? []).filter((a) => query.get("include_archived") === "true" || !a.archived_at)),
  );
  on("GET", "orgs/:org/projects/:project/limits", ({ query }: { query: URLSearchParams }) =>
    page(
      (d.limits ?? []).filter(
        (l) =>
          (!query.get("level") || l.level === query.get("level")) &&
          (!query.get("key") || l.subject.id === query.get("key")) &&
          (!query.get("user") || l.subject.id === query.get("user")) &&
          (!query.get("service_account") || l.subject.id === query.get("service_account")),
      ),
    ),
  );
  on("GET", "orgs/:org/projects/:project/models", page(d.models ?? []));
  on("GET", "orgs/:org/projects/:project/aliases", page(d.aliases ?? []));
  on("GET", "orgs/:org/projects/:project/members/:user/models", access());
  on("GET", "orgs/:org/projects/:project/service-accounts/:account/models", access());
  on("GET", "orgs/:org/projects/:project/keys/:key/models", access());
  on("GET", "orgs/:org/projects/:project/rights/:right/grantors", page([memberUser({ id: "usr_lin", name: "Lin Wei" })]));
  on("GET", "orgs/:org/rights/:right/grantors", page([memberUser({ id: "usr_lin", name: "Lin Wei" })]));
}

interface ModelsApi {
  models?: CatalogueModelView[];
  hits?: SearchHit[];
  previews?: Preview[];
}

export function modelsApi({ models = [], hits = [], previews = [] }: ModelsApi = {}): void {
  on(
    "GET",
    "orgs/:org/projects/:project/models/:model",
    ({ params }: { params: Record<string, string> }) => models.find((m) => m.id === params.model) ?? apiError(404, "not_found", "No such model."),
  );
  on("GET", "orgs/:org/projects/:project/models", page(models));
  on("GET", "orgs/:org/projects/:project/models/search", ({ query }: { query: URLSearchParams }) =>
    page(
      hits.filter(
        (h) =>
          h.provider === query.get("provider") &&
          (!query.get("q") || h.model_id.includes(query.get("q") ?? "")) &&
          (query.get("include_deprecated") === "true" || !h.deprecated),
      ),
    ),
  );
  on(
    "GET",
    "orgs/:org/projects/:project/models/preview",
    ({ query }: { query: URLSearchParams }) =>
      previews.find((p) => p.provider === query.get("provider") && p.model_id === query.get("model")) ??
      apiError(404, "not_found", "models.dev doesn't list it."),
  );
}
