import type { AgentCatalogue, AgentPresetView, AgentView, GrantBody, Grantable, OrgAgentView } from "~/api/generated/types";
import agentCatalogue from "./agent-catalogue.json";
import { ORG, OTHER_PROJECT, PROJECT, USER } from "../fixtures";

export const AGENT = "agt_release";
export const agents = agentCatalogue as AgentCatalogue;

function preset(id: string): AgentPresetView {
  const found = agents.presets.find((p) => p.id === id);
  if (!found) throw new Error(`no preset ${id}`);
  return found;
}

export function grantable(overrides: Partial<Grantable> = {}): Grantable {
  return {
    instance: [],
    allowed: true,
    instance_max: "configure",
    account: agents.account.categories.flatMap((c) => c.rights.map((r) => r.id)),
    orgs: [
      {
        id: ORG,
        name: "Northwind",
        avatar_url: null,
        allowed: true,
        max: "full",
        rights: preset("full").org,
        every_project: preset("full").project,
        projects: [
          { id: PROJECT, name: "Checkout", is_default: true, avatar_url: null, rights: preset("full").project },
          { id: OTHER_PROJECT, name: "Support triage", is_default: false, avatar_url: null, rights: preset("observer").project },
        ],
      },
      {
        id: "org_contoso",
        name: "Contoso",
        avatar_url: null,
        allowed: false,
        max: "observer",
        rights: ["org.view"],
        every_project: [],
        projects: [],
      },
    ],
    ...overrides,
  };
}

export function grantBody(overrides: Partial<GrantBody> = {}): GrantBody {
  const builder = preset("builder");
  return {
    preset: "builder",
    customized: false,
    instance: [],
    account: builder.account,
    orgs: [
      { org: ORG, included: true, rights: builder.org, projects: { all: true, rights: builder.project, selected: [] } },
      { org: "org_contoso", included: false, rights: [], projects: { all: true, rights: [], selected: [] } },
    ],
    future_orgs: null,
    ...overrides,
  };
}

export function agentView(overrides: Partial<AgentView> = {}): AgentView {
  return {
    id: AGENT,
    name: "Release helper",
    harness: "claude_code",
    model: null,
    client: "Claude Code",
    grant: grantBody(),
    created_at: "2026-10-01T12:00:00Z",
    last_used_at: "2026-10-15T11:00:00Z",
    expires_at: "2026-12-30T12:00:00Z",
    ...overrides,
  };
}

export function orgAgent(overrides: Partial<OrgAgentView> = {}): OrgAgentView {
  return {
    id: AGENT,
    name: "Release helper",
    harness: "codex",
    model: null,
    user: { id: USER, name: "Ada Okafor", email: "ada@northwind.test", state: "active", avatar_url: null },
    created_at: "2026-10-01T12:00:00Z",
    last_used_at: null,
    expires_at: null,
    ...overrides,
  };
}
