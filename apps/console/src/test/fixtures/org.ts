import { act, screen, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { showTooltipsAtOnce } from "~/test/tooltip-delay";
import type {
  CreatedKey_OrgKeyView,
  CredentialView,
  InviteView,
  LogJobView,
  MemberRights,
  MemberView,
  OrgKeyView,
  OrgLogSettings,
  SecurityView,
  ServiceAccountView,
  SsoView,
  StorageView,
  UsageRow,
} from "~/api/generated/types";
import { ALL_ORG_RIGHTS, ALL_PROJECT_RIGHTS, NOW, PROJECT, USER } from "../fixtures";
import { apiError, on, page, type ApiRequest } from "../network";

export function member(overrides: Partial<Omit<MemberView, "user">> & { user?: Partial<MemberView["user"]> } = {}): MemberView {
  const { user, ...rest } = overrides;
  return {
    id: `mem_${user?.id ?? USER}`,
    joined_at: NOW,
    grant: {
      role: { id: "owner", kind: "builtin", name: "Owner" },
      preset: "owner",
      rights: ALL_ORG_RIGHTS,
      project_rights: ALL_PROJECT_RIGHTS,
      rights_added: [],
      rights_removed: [],
    },
    ...rest,
    user: { id: USER, name: "Ada Okafor", email: "owner@example.com", state: "active", avatar_url: null, ...user },
  };
}

export function roleGrant(id: string, name: string, rights: string[] = ["org.view"], projectRights: string[] = []): MemberView["grant"] {
  return { role: { id, kind: "builtin", name }, preset: id, rights, project_rights: projectRights, rights_added: [], rights_removed: [] };
}

export function invite(overrides: Partial<InviteView> = {}): InviteView {
  return {
    id: "inv_1",
    email: "grace@example.com",
    status: "pending",
    created_at: NOW,
    expires_at: "2026-11-05T12:00:00Z",
    invited_by: USER,
    role: { id: "member", kind: "builtin", name: "Member" },
    preset: "member",
    project_roles: [],
    ...overrides,
  };
}

export function platformKey(overrides: Partial<OrgKeyView> = {}): OrgKeyView {
  return {
    id: "opk_terraform",
    name: "Terraform",
    redacted: "kso_org_…a1b2",
    state: "active",
    created_at: NOW,
    created_by: { kind: "user", id: USER },
    expires_at: null,
    last_used_at: null,
    ...overrides,
  };
}

export function createdPlatformKey(overrides: Partial<CreatedKey_OrgKeyView> = {}): CreatedKey_OrgKeyView {
  return { ...platformKey(), secret: "kso_org_secret_value", ...overrides };
}

export function credential(overrides: Partial<CredentialView> = {}): CredentialView {
  return {
    id: "cred_openai",
    name: "OpenAI production",
    provider: "openai",
    is_default: true,
    key_id: "mk_1",
    last4: "9f3c",
    projects: [],
    created_at: NOW,
    updated_at: NOW,
    created_by: { kind: "user", id: USER },
    last_used_at: null,
    ...overrides,
  };
}

export function sso(overrides: Partial<SsoView> = {}): SsoView {
  return {
    issuer: "https://login.northwind.test",
    client_id: "client-123",
    domains: ["northwind.test"],
    key_id: "mk_sso",
    redirect_url: "https://console.northwind.test/sso/callback",
    name: null,
    auto_redirect: false,
    sign_in_url: "https://console.northwind.test/sign-in?org=org_northwind",
    ...overrides,
  };
}

export function security(overrides: Partial<SecurityView> = {}): SecurityView {
  return { require_2fa: false, require_sso: false, sso: null, sso_default_role: null, max_key_lifetime_days: null, ...overrides };
}

export function logSettings(overrides: Partial<OrgLogSettings> = {}): OrgLogSettings {
  return {
    content_logging: true,
    metadata_retention_days: 90,
    content_retention_days: 30,
    max_metadata_retention_days: 365,
    max_content_retention_days: 180,
    ...overrides,
  };
}

export function storage(overrides: Partial<StorageView> = {}): StorageView {
  return {
    entries: 1500,
    logical_bytes: 2_000_000,
    estimated_disk_bytes: 1_000_000,
    projects: [
      { project_id: PROJECT, entries: 1200, entries_with_content: 800, logical_bytes: 1_500_000, estimated_disk_bytes: 700_000 },
      { project_id: "proj_gone", entries: 300, entries_with_content: 100, logical_bytes: 500_000, estimated_disk_bytes: 300_000 },
    ],
    ...overrides,
  };
}

export function logJob(overrides: Partial<LogJobView> = {}): LogJobView {
  return {
    id: "job_1",
    kind: "deletion",
    state: "done",
    from: "2026-10-01T00:00:00Z",
    to: "2026-10-03T00:00:00Z",
    project_id: PROJECT,
    created_at: NOW,
    done_at: NOW,
    last_error: null,
    ...overrides,
  };
}

export function memberRights(overrides: Partial<MemberRights> = {}): MemberRights {
  return {
    user_id: USER,
    org: { role: { id: "owner", kind: "builtin", name: "Owner" }, rights: [{ right: "org.view", sources: [{ kind: "role" }] }], removed: [] },
    every_project: [],
    projects: [],
    ...overrides,
  };
}

export function usageRow(key: string, overrides: Partial<UsageRow> = {}): UsageRow {
  return {
    key,
    requests: 10,
    errors: 0,
    input_tokens: 100,
    output_tokens: 50,
    cache_read_tokens: 0,
    cache_write_tokens: 0,
    reasoning_tokens: 0,
    cost: null,
    ...overrides,
  };
}

export function serviceAccount(overrides: Partial<ServiceAccountView> = {}): ServiceAccountView {
  return {
    id: "sa_bot",
    name: "Nightly bot",
    project_id: PROJECT,
    created_at: NOW,
    created_by: USER,
    archived_at: null,
    grant: { role: { id: "developer", kind: "builtin", name: "Developer" }, preset: "developer", rights: [], rights_added: [], rights_removed: [] },
    ...overrides,
  };
}

export const paged = page;

export const OTHER_USER = "usr_grace";
export const THIRD_USER = "usr_linus";

export function notifications() {
  return within(screen.getByRole("region", { name: "Notifications" }));
}

export async function hoverReason(user: UserEvent, element: HTMLElement, text: string | RegExp): Promise<HTMLElement> {
  act(() => showTooltipsAtOnce(true));
  try {
    await user.hover(element);
    const tip = await screen.findByText(text);
    await user.unhover(element);
    return tip;
  } finally {
    act(() => showTooltipsAtOnce(false));
  }
}

export function membersData(list: MemberView[], rights: (user: string) => MemberRights = (user) => memberRights({ user_id: user })): void {
  on("GET", "orgs/:org/members", paged(list));
  on(
    "GET",
    "orgs/:org/members/:user",
    ({ params }: ApiRequest) => list.find((m) => m.user.id === params.user) ?? apiError(404, "not_found", "No such member."),
  );
  on("GET", "orgs/:org/members/:user/rights", ({ params }: ApiRequest) => rights(params.user ?? ""));
}

export async function fill(user: UserEvent, field: HTMLElement, text: string): Promise<void> {
  await user.click(field);
  await user.paste(text);
}
