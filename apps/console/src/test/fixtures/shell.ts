import type { AuditEventView, CatalogueModelView, CreatedKey_KeyView, CredentialView, KeyView, UsageRow } from "~/api/generated/types";
import { NOW, PROJECT, USER } from "../fixtures";

function key(overrides: Partial<KeyView> = {}): KeyView {
  return {
    id: "key_web",
    name: "checkout-web",
    redacted: "sk-kso-ab…cd",
    state: "active",
    access: "inference",
    created_at: NOW,
    created_by: { kind: "user", id: USER },
    principal: { kind: "user", id: USER },
    project_id: PROJECT,
    expires_at: null,
    last_used_at: null,
    ...overrides,
  };
}

export function createdKey(overrides: Partial<CreatedKey_KeyView> = {}): CreatedKey_KeyView {
  return { ...key(), secret: "sk-kso-secret-value-1234567890", ...overrides };
}

export function model(overrides: Partial<CatalogueModelView> = {}): CatalogueModelView {
  return {
    id: "cm_gpt",
    model_id: "gpt-5-mini",
    provider: "openai",
    aliases: [],
    custom: false,
    listed: true,
    egress_dialect: "openai",
    effective: { display_name: "GPT-5 mini" },
    overrides: {},
    created_at: NOW,
    updated_at: NOW,
    created_by: { kind: "user", id: USER },
    ...overrides,
  };
}

export function usage(overrides: Partial<UsageRow> = {}): UsageRow {
  return {
    key: "2026-10-14",
    requests: 100,
    errors: 5,
    input_tokens: 1000,
    output_tokens: 500,
    reasoning_tokens: 100,
    cache_read_tokens: 200,
    cache_write_tokens: 50,
    cost: "1.25",
    ...overrides,
  };
}

export function auditEvent(overrides: Partial<AuditEventView> = {}): AuditEventView {
  return {
    id: "evt_1",
    action: "key.created",
    at: NOW,
    actor: { kind: "user", id: USER },
    target_kind: "key",
    target_id: "key_web",
    ip: "203.0.113.7",
    user_agent: "Mozilla/5.0",
    request_id: "req_123",
    before: null,
    after: null,
    ...overrides,
  };
}

export function credential(overrides: Partial<CredentialView> = {}): CredentialView {
  return {
    id: "cred_prod",
    key_id: "key-1",
    name: "Production",
    provider: "openai",
    is_default: true,
    last4: "abcd",
    projects: [],
    created_at: NOW,
    updated_at: NOW,
    created_by: { kind: "user", id: USER },
    last_used_at: null,
    ...overrides,
  };
}
