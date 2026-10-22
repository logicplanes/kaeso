import type {
  InstanceOrgUsage,
  InstanceOrgView,
  InstanceUserView,
  KeyView,
  Me,
  MemberView,
  ModelsDevStatus,
  SessionListItem,
  ReleaseView,
  SettingsView,
  SystemView,
  UpdateJobView,
  UpdatesView,
} from "~/api/generated/types";
import { NOW, ORG, PROJECT, USER } from "../fixtures";

export function me(overrides: Partial<Me> = {}): Me {
  return {
    id: USER,
    name: "Ada Okafor",
    email: "owner@example.com",
    state: "active",
    is_instance_admin: true,
    avatar_url: null,
    created_at: NOW,
    has_password: true,
    locale: "auto",
    recovery_codes_left: 10,
    totp_enabled: false,
    ...overrides,
  };
}

export function instanceSettings(overrides: Partial<SettingsView> = {}): SettingsView {
  return {
    account_creation: "invitation_only",
    allowed_domains: [],
    content_retention_days: 30,
    metadata_retention_days: 90,
    max_content_retention_days: 90,
    max_metadata_retention_days: 365,
    org_creation: "instance_admins",
    org_deletion: true,
    org_deletion_grace_days: 7,
    timezone: "UTC",
    mail: { transport: "smtp", host: "mail.example.com", port: 587, from: "kaeso@example.com" },
    sign_in: { password: true, magic_link: true },
    ...overrides,
  };
}

export function system(overrides: Partial<SystemView> = {}): SystemView {
  return {
    status: "ready",
    version: "40.4.2",
    postgres: "ok",
    clickhouse: "ok",
    valkey: "ok",
    log_buffer: { rows: 12, dropped: 0 },
    storage: { postgres_bytes: 2_097_152, clickhouse_bytes_on_disk: 10_485_760, request_log_rows: 48_210 },
    ...overrides,
  };
}

export function modelsDev(overrides: Partial<ModelsDevStatus> = {}): ModelsDevStatus {
  return {
    fetched_at: NOW,
    interval_seconds: 3600,
    last_attempt_at: NOW,
    last_error: null,
    model_count: 412,
    source: "remote",
    sync_enabled: true,
    url: "https://models.dev/api.json",
    ...overrides,
  };
}

export function instanceUser(overrides: Partial<InstanceUserView> = {}): InstanceUserView {
  return {
    id: "usr_grace",
    name: "Grace Hopper",
    email: "grace@example.com",
    state: "active",
    is_instance_admin: false,
    avatar_url: null,
    created_at: NOW,
    totp_enabled: false,
    managed_by: null,
    ...overrides,
  };
}

export function instanceOrg(overrides: Partial<InstanceOrgView> = {}): InstanceOrgView {
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
    members: 4,
    owners: 1,
    projects: 2,
    ...overrides,
  };
}

export function orgUsage(overrides: Partial<InstanceOrgUsage> = {}): InstanceOrgUsage {
  return { requests_30d: 3000, tokens_30d: 1_500_000, cost_30d: "12.50", log_entries: 3000, log_logical_bytes: 5_242_880, ...overrides };
}

export function member(overrides: { id?: string; name?: string; email?: string | null; state?: string; role?: "owner" | "member" } = {}): MemberView {
  const role = overrides.role ?? "member";
  return {
    id: `mem_${overrides.id ?? "grace"}`,
    joined_at: NOW,
    managed: false,
    user: {
      id: overrides.id ?? "usr_grace",
      name: overrides.name ?? "Grace Hopper",
      email: overrides.email === undefined ? "grace@example.com" : overrides.email,
      state: overrides.state ?? "active",
      avatar_url: null,
    },
    grant: { role: { id: role, kind: "builtin", name: role === "owner" ? "Owner" : "Member" }, rights: [], rights_added: [], rights_removed: [] },
  };
}

export function sessionItem(overrides: Partial<SessionListItem> = {}): SessionListItem {
  return {
    id: "ses_current",
    method: "password",
    second_factor: false,
    signed_in_at: NOW,
    last_used_at: NOW,
    expires_at: "2026-11-14T12:00:00Z",
    created_at: NOW,
    current: true,
    ip: "203.0.113.7",
    user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15",
    ...overrides,
  };
}

export function key(overrides: Partial<KeyView> = {}): KeyView {
  return {
    id: "key_support",
    name: "support-bot",
    access: "inference",
    created_at: NOW,
    created_by: { kind: "user", id: USER },
    principal: { kind: "user", id: USER },
    project_id: PROJECT,
    redacted: "sk-kso-AbC123…wxyz",
    state: "active",
    expires_at: null,
    last_used_at: null,
    ...overrides,
  };
}

export function release(overrides: Partial<ReleaseView> = {}): ReleaseView {
  return {
    version: "40.4.3",
    published_at: NOW,
    prerelease: false,
    major: false,
    manual_steps: false,
    notes: "### Fixed\n\n- Streams keep their last chunk when a provider closes early.",
    url: "https://github.com/logicplanes/kaeso/releases/tag/v40.4.3",
    ...overrides,
  };
}

export function updateSettings(overrides: Partial<UpdatesView["settings"]> = {}): UpdatesView["settings"] {
  return {
    check: true,
    channel: "stable",
    mode: "manual",
    schedule: { frequency: "daily", weekday: "monday", hour: 3, timezone: "UTC" },
    next_auto_update_at: null,
    ...overrides,
  };
}

export function updates(overrides: Partial<UpdatesView> = {}): UpdatesView {
  return {
    current: "40.4.2",
    state: "up_to_date",
    manual: null,
    latest: null,
    releases: [],
    can_apply: false,
    settings: updateSettings(),
    check: { last_checked_at: NOW, last_success_at: NOW, next_check_at: NOW, error: null },
    updater: { state: "ready", version: "40.4.2" },
    job: null,
    upgrade_guide: "https://docs.kaeso.app/self-hosting/upgrades",
    ...overrides,
  };
}

export function available(overrides: Partial<UpdatesView> = {}): UpdatesView {
  const r = release();
  return updates({ state: "available", latest: r, releases: [r], can_apply: true, ...overrides });
}

export function updateJob(overrides: Partial<UpdateJobView> = {}): UpdateJobView {
  return {
    id: "upd_01k7a3f8m2xq4v9c5r6t8w0yz1",
    from: "40.4.2",
    to: "40.4.3",
    state: "running",
    step: "verifying",
    reason: null,
    detail: null,
    started_by: { id: USER, name: "Ada Okafor" },
    automatic: false,
    started_at: NOW,
    finished_at: null,
    ...overrides,
  };
}
