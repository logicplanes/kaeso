import { t } from "~/i18n";
import { joinList } from "./labels";

export const AUDIT_ACTIONS: Record<string, string[]> = {
  alias: ["alias.create", "alias.update", "alias.delete"],
  audit_log: ["audit_log.export"],
  catalogue_model: ["catalogue_model.create", "catalogue_model.update", "catalogue_model.price_change", "catalogue_model.check", "catalogue_model.delete"],
  credential: ["credential.create", "credential.replace", "credential.rename", "credential.default", "credential.assign", "credential.delete"],
  instance: ["instance.setup", "instance.invite", "instance.settings_update"],
  invite: ["invite.create", "invite.resend", "invite.revoke", "invite.accept"],
  key: ["key.create", "key.disable", "key.enable", "key.archive"],
  limit: ["limit.create", "limit.update", "limit.delete", "limit.threshold_reached"],
  logs: ["logs.settings_update", "logs.project_content_logging", "logs.export", "logs.delete"],
  member: ["member.rights_update", "member.2fa_reset", "member.remove", "member.join_by_sso"],
  model_access: ["model_access.update"],
  models_dev: ["models_dev.sync", "models_dev.price_change", "models_dev.model_delisted"],
  org: [
    "org.create",
    "org.update",
    "org.ownership_transfer",
    "org.owner_appointed",
    "org.owner_appointed_by_instance_admin",
    "org.schedule_deletion",
    "org.restore",
    "org.delete_final",
  ],
  platform_key: ["platform_key.create", "platform_key.disable", "platform_key.enable", "platform_key.archive"],
  project: [
    "project.create",
    "project.update",
    "project.key_policy",
    "project.archive",
    "project.member_add",
    "project.member_rights_update",
    "project.member_remove",
  ],
  role: ["role.create", "role.update", "role.delete"],
  security: ["security.sso_update", "security.sso_delete", "security.require_sso", "security.require_2fa", "security.key_policy"],
  service_account: ["service_account.create", "service_account.update", "service_account.archive"],
  user: [
    "user.signup",
    "user.create_by_invite",
    "user.create_by_sso",
    "user.update",
    "user.email_change",
    "user.password_change",
    "user.password_reset",
    "user.totp_enable",
    "user.totp_disable",
    "user.recovery_codes_regenerate",
    "user.2fa_reset",
    "user.sign_in_link",
    "user.sessions_revoke",
    "user.notifications",
    "user.deactivate",
    "user.reactivate",
    "user.instance_admin",
    "user.delete",
  ],
};

export function auditActionOptions(): { value: string; label: string; description?: string }[] {
  return Object.entries(AUDIT_ACTIONS).flatMap(([prefix, actions]) => [
    { value: `${prefix}.`, label: `${prefix}.*`, description: t("format.audit.everyAction", "Every action of this kind") },
    ...actions.map((a) => ({ value: a, label: a })),
  ]);
}

const TARGET_KINDS: Record<string, () => string> = {
  user: () => t("format.targetKind.user", "User"),
  org: () => t("common.organization", "Organization"),
  invite: () => t("format.targetKind.invite", "Invitation"),
  project: () => t("common.project", "Project"),
  service_account: () => t("format.targetKind.serviceAccount", "Service account"),
  limit: () => t("format.targetKind.limit", "Limit"),
  credential: () => t("format.targetKind.credential", "Provider key"),
  catalogue_model: () => t("common.model", "Model"),
  alias: () => t("format.targetKind.alias", "Alias"),
  platform_key: () => t("format.targetKind.platformKey", "Platform key"),
  key: () => t("format.targetKind.key", "Key"),
};
export const targetKindLabel = (kind: string) => TARGET_KINDS[kind]?.() ?? kind;

type Json = Record<string, unknown> | null | undefined;
const list = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : []);
const obj = (v: unknown): Json => (v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : null);

export function rightsChangeSummary(before: unknown, after: unknown, name: (right: string) => string): string | null {
  const b = obj(before);
  const a = obj(after);
  const parts: string[] = [];
  for (const key of ["rights", "project_rights"] as const) {
    const was = new Set(list(b?.[key]));
    const now = new Set(list(a?.[key]));
    const added = [...now].filter((r) => !was.has(r)).map((r) => `+ ${name(r)}`);
    const removed = [...was].filter((r) => !now.has(r)).map((r) => `− ${name(r)}`);
    if (!added.length && !removed.length) continue;
    const changes = joinList([...added, ...removed]);
    parts.push(key === "rights" ? changes : t("format.audit.everyProject", "in every project {changes}", { changes }));
  }
  const roleOf = (j: Json) => {
    const role = obj(j?.["role"]);
    return typeof role?.["name"] === "string"
      ? role["name"]
      : typeof j?.["name"] === "string"
        ? j["name"]
        : typeof j?.["preset"] === "string"
          ? j["preset"]
          : null;
  };
  const was = roleOf(b);
  const now = roleOf(a);
  const lead = was && now && was !== now ? `${was} → ${now}` : (now ?? was);
  if (!parts.length) return was && now && was !== now ? lead : null;
  const changes = parts.join(t("format.audit.partSeparator", "; "));
  return lead ? t("format.audit.roleRights", "{role}: {changes}", { role: lead, changes }) : changes;
}
