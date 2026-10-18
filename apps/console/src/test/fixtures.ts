import type { Membership, MyMembership, OrgView, ProjectRightsView, ProjectView, RightsCatalogue, RoleView, WhoAmI } from "~/api/generated/types";
import rightsCatalogue from "./fixtures/rights.json";
import roleList from "./fixtures/roles.json";
import { noContent, on, page } from "./network";

export const ORG = "org_northwind";
export const PROJECT = "proj_checkout";
export const OTHER_PROJECT = "proj_support";
export const USER = "usr_ada";
export const NOW = "2026-10-15T12:00:00Z";

export const catalogue = rightsCatalogue as RightsCatalogue;
export const roles = roleList.data as RoleView[];

const idsOf = (scope: "org" | "project") => catalogue[scope].categories.flatMap((c) => c.rights.map((r) => r.id));
export const ALL_ORG_RIGHTS = idsOf("org");
export const ALL_PROJECT_RIGHTS = idsOf("project");

export function user(overrides: Partial<WhoAmI["user"]> = {}): WhoAmI["user"] {
  return {
    id: USER,
    name: "Ada Okafor",
    email: "owner@example.com",
    state: "active",
    is_instance_admin: true,
    avatar_url: null,
    created_at: NOW,
    ...overrides,
  };
}

export function membership(overrides: Partial<Membership> = {}): Membership {
  return {
    org_id: ORG,
    org_name: "Northwind Labs",
    org_state: "active",
    org_deletion_due_at: null,
    org_avatar_url: null,
    role: { id: "owner", kind: "builtin", name: "Owner" },
    preset: "owner",
    rights: ALL_ORG_RIGHTS,
    project_rights: ALL_PROJECT_RIGHTS,
    access: "all",
    ...overrides,
  };
}

export function whoAmI(overrides: Partial<WhoAmI> = {}): WhoAmI {
  return {
    user: user(),
    totp_enabled: false,
    session: { id: "ses_current", method: "password", second_factor: false, signed_in_at: NOW, last_used_at: NOW, expires_at: "2026-11-14T12:00:00Z" },
    memberships: [membership()],
    can_create_orgs: true,
    org_deletion: { allowed: true, grace_days: 7 },
    locale: "auto",
    timezone: "UTC",
    ...overrides,
  } as WhoAmI;
}

export function myProject(overrides: Partial<MyMembership["projects"][number]> = {}): MyMembership["projects"][number] {
  return {
    id: PROJECT,
    name: "Checkout assistant",
    is_default: true,
    avatar_url: null,
    role: { id: "admin", kind: "builtin", name: "Admin" },
    preset: "admin",
    ...overrides,
  };
}

export function myMembership(overrides: Partial<MyMembership> = {}): MyMembership {
  return {
    org_id: ORG,
    org_name: "Northwind Labs",
    org_state: "active",
    org_deletion_due_at: null,
    org_avatar_url: null,
    role: { id: "owner", kind: "builtin", name: "Owner" },
    preset: "owner",
    projects: [myProject(), myProject({ id: OTHER_PROJECT, name: "Support triage", is_default: false })],
    ...overrides,
  };
}

export function project(overrides: Partial<ProjectView> = {}): ProjectView {
  return {
    id: PROJECT,
    org_id: ORG,
    name: "Checkout assistant",
    is_default: true,
    content_logging: true,
    created_at: NOW,
    archived_at: null,
    avatar_url: null,
    max_key_lifetime_days: null,
    ...overrides,
  };
}

export function org(overrides: Partial<OrgView> = {}): OrgView {
  return {
    id: ORG,
    name: "Northwind Labs",
    state: "active",
    content_logging: true,
    metadata_retention_days: 90,
    content_retention_days: 30,
    require_2fa: false,
    require_sso: false,
    created_at: NOW,
    avatar_url: null,
    deletion_due_at: null,
    max_key_lifetime_days: null,
    sso_default_preset: null,
    ...overrides,
  };
}

function myRights(rights: readonly string[] = ALL_PROJECT_RIGHTS, project = PROJECT): ProjectRightsView {
  return {
    project_id: project,
    user_id: USER,
    role: { id: "admin", kind: "builtin", name: "Admin" },
    rights: rights.map((right) => ({ right, sources: [{ kind: "owner" }] })),
  } as ProjectRightsView;
}

export interface SignedInOptions {
  session?: WhoAmI;
  memberships?: MyMembership[];
  projectRights?: readonly string[];
  projects?: ProjectView[];
}

export function signedIn(options: SignedInOptions = {}): WhoAmI {
  const session = options.session ?? whoAmI();
  const memberships = options.memberships ?? [myMembership()];
  const projects = options.projects ?? [project(), project({ id: OTHER_PROJECT, name: "Support triage", is_default: false })];
  on("GET", "auth/session", session);
  on("GET", "me/memberships", { data: memberships });
  on("GET", "me/whats-new", { current: "40.4.2", seen: "40.4.2", releases: [] });
  on("POST", "me/whats-new/seen", noContent);
  on("GET", "orgs/:org/rights", catalogue);
  on("GET", "orgs/:org/roles", page(roles));
  on("GET", "orgs/:org", ({ params }: { params: Record<string, string> }) => org({ id: params.org ?? ORG }));
  on("GET", "orgs/:org/projects", page(projects));
  on("GET", "orgs/:org/projects/:project", ({ params }: { params: Record<string, string> }) => projects.find((p) => p.id === params.project) ?? project());
  on("GET", "orgs/:org/projects/:project/my-rights", ({ params }: { params: Record<string, string> }) =>
    myRights(options.projectRights ?? ALL_PROJECT_RIGHTS, params.project),
  );
  return session;
}

const BUILTIN_ROLES = { owner: "Owner", admin: "Admin", auditor: "Auditor", member: "Member" } as const;
type BuiltinRole = keyof typeof BUILTIN_ROLES;

export const without = (...rights: string[]): string[] => ALL_PROJECT_RIGHTS.filter((r) => !rights.includes(r));

export const ADMIN_ORG_RIGHTS = ALL_ORG_RIGHTS.filter((r) => r !== "billing.manage");

export function withRights(
  rights: string[],
  { role = "owner", session = {}, projectRights }: { role?: BuiltinRole; session?: Partial<WhoAmI>; projectRights?: readonly string[] } = {},
): WhoAmI {
  const grant = role === "owner" ? {} : { role: { id: role, kind: "builtin" as const, name: BUILTIN_ROLES[role] }, preset: role };
  return signedIn({ session: whoAmI({ memberships: [membership({ rights, ...grant })], ...session }), ...(projectRights ? { projectRights } : {}) });
}

export const asAdmin = (rights: string[] = ADMIN_ORG_RIGHTS, projectRights?: readonly string[]): WhoAmI =>
  withRights(rights, { role: "admin", ...(projectRights ? { projectRights } : {}) });

export function signedOut(): void {
  on("GET", "auth/session", () => new Response(JSON.stringify({ error: { type: "authentication", message: "Sign in to continue." } }), { status: 401 }));
}
