import { t } from "~/i18n";
import { joinList } from "./labels";

const KIND_LABELS: Record<string, () => string> = {
  agent: () => t("format.audit.kind.agent", "Agents"),
  alias: () => t("format.audit.kind.alias", "Aliases"),
  audit_log: () => t("format.audit.kind.auditLog", "Audit log"),
  catalogue_model: () => t("format.audit.kind.catalogueModel", "Models"),
  credential: () => t("format.audit.kind.credential", "Provider keys"),
  custom_provider: () => t("format.audit.kind.customProvider", "Custom providers"),
  instance: () => t("format.audit.kind.instance", "Instance"),
  invite: () => t("format.audit.kind.invite", "Invitations"),
  key: () => t("format.audit.kind.key", "Keys"),
  limit: () => t("format.audit.kind.limit", "Limits"),
  logs: () => t("format.audit.kind.logs", "Request logs"),
  member: () => t("format.audit.kind.member", "Members"),
  kpmr: () => t("format.audit.kind.kpmr", "KPMR"),
  model_access: () => t("format.audit.kind.modelAccess", "Model access"),
  models_dev: () => t("format.audit.kind.modelsDev", "models.dev"),
  org: () => t("format.audit.kind.org", "Organization"),
  platform_key: () => t("format.audit.kind.platformKey", "Platform keys"),
  project: () => t("format.audit.kind.project", "Projects"),
  response_cache: () => t("format.audit.kind.responseCache", "Response cache"),
  role: () => t("format.audit.kind.role", "Roles"),
  security: () => t("format.audit.kind.security", "Security"),
  service_account: () => t("format.audit.kind.serviceAccount", "Service accounts"),
  user: () => t("format.audit.kind.user", "Users"),
};

const ACTION_LABELS: Record<string, () => string> = {
  "alias.create": () => t("format.audit.action.aliasCreate", "Alias created"),
  "alias.update": () => t("format.audit.action.aliasUpdate", "Alias changed"),
  "alias.delete": () => t("format.audit.action.aliasDelete", "Alias deleted"),
  "agent.authorize": () => t("format.audit.action.agentAuthorize", "Agent authorized"),
  "agent.update": () => t("format.audit.action.agentUpdate", "Agent renamed or renewed"),
  "agent.rights_update": () => t("format.audit.action.agentRightsUpdate", "Agent rights changed"),
  "agent.revoke": () => t("format.audit.action.agentRevoke", "Agent revoked"),
  "agent.org_access_revoke": () => t("format.audit.action.agentOrgAccessRevoke", "Organization closed to an agent"),
  "audit_log.export": () => t("format.audit.action.auditLogExport", "Audit log exported"),
  "catalogue_model.create": () => t("format.audit.action.catalogueModelCreate", "Model added"),
  "catalogue_model.update": () => t("format.audit.action.catalogueModelUpdate", "Model values changed"),
  "catalogue_model.price_change": () => t("format.audit.action.catalogueModelPriceChange", "Model price changed"),
  "catalogue_model.check": () => t("format.audit.action.catalogueModelCheck", "Models checked with the provider"),
  "catalogue_model.delete": () => t("format.audit.action.catalogueModelDelete", "Model removed"),
  "credential.create": () => t("format.audit.action.credentialCreate", "Provider key added"),
  "credential.replace": () => t("format.audit.action.credentialReplace", "Provider key replaced"),
  "credential.rename": () => t("format.audit.action.credentialRename", "Provider key renamed"),
  "credential.default": () => t("format.audit.action.credentialDefault", "Provider key made the default"),
  "credential.assign": () => t("format.audit.action.credentialAssign", "Provider key assigned to projects"),
  "credential.delete": () => t("format.audit.action.credentialDelete", "Provider key deleted"),
  "custom_provider.create": () => t("format.audit.action.customProviderCreate", "Custom provider added"),
  "custom_provider.update": () => t("format.audit.action.customProviderUpdate", "Custom provider changed"),
  "custom_provider.delete": () => t("format.audit.action.customProviderDelete", "Custom provider deleted"),
  "instance.setup": () => t("format.audit.action.instanceSetup", "Kaeso set up"),
  "instance.invite": () => t("format.audit.action.instanceInvite", "User invited to the install"),
  "instance.settings_update": () => t("format.audit.action.instanceSettingsUpdate", "Instance settings changed"),
  "instance.admin_sso_update": () => t("format.audit.action.instanceAdminSsoUpdate", "Single sign-on for instance admins changed"),
  "instance.update_check": () => t("format.audit.action.instanceUpdateCheck", "Checked for updates"),
  "instance.update_settings": () => t("format.audit.action.instanceUpdateSettings", "Update settings changed"),
  "instance.update_start": () => t("format.audit.action.instanceUpdateStart", "Update started"),
  "instance.update_finish": () => t("format.audit.action.instanceUpdateFinish", "Update finished"),
  "instance.agents_policy": () => t("format.audit.action.instanceAgentsPolicy", "Instance agent policy changed"),
  "invite.create": () => t("format.audit.action.inviteCreate", "Invitation sent"),
  "invite.resend": () => t("format.audit.action.inviteResend", "Invitation sent again"),
  "invite.revoke": () => t("format.audit.action.inviteRevoke", "Invitation revoked"),
  "invite.accept": () => t("format.audit.action.inviteAccept", "Invitation accepted"),
  "key.create": () => t("format.audit.action.keyCreate", "Key created"),
  "key.disable": () => t("format.audit.action.keyDisable", "Key disabled"),
  "key.enable": () => t("format.audit.action.keyEnable", "Key enabled"),
  "key.archive": () => t("format.audit.action.keyArchive", "Key archived"),
  "key.rotate": () => t("format.audit.action.keyRotate", "Key rotated"),
  "limit.create": () => t("format.audit.action.limitCreate", "Limit added"),
  "limit.update": () => t("format.audit.action.limitUpdate", "Limit changed"),
  "limit.delete": () => t("format.audit.action.limitDelete", "Limit removed"),
  "limit.threshold_reached": () => t("format.audit.action.limitThresholdReached", "Limit alert threshold reached"),
  "logs.settings_update": () => t("format.audit.action.logsSettingsUpdate", "Logging settings changed"),
  "logs.project_content_logging": () => t("format.audit.action.logsProjectContentLogging", "A project's content logging changed"),
  "logs.export": () => t("format.audit.action.logsExport", "Request logs exported"),
  "logs.delete": () => t("format.audit.action.logsDelete", "Request logs deleted"),
  "member.rights_update": () => t("format.audit.action.memberRightsUpdate", "Member's role or rights changed"),
  "member.remove": () => t("format.audit.action.memberRemove", "Member removed"),
  "member.join_by_sso": () => t("format.audit.action.memberJoinBySso", "Member joined through single sign-on"),
  "model_access.update": () => t("format.audit.action.modelAccessUpdate", "Model access changed"),
  "kpmr.price_change": () => t("format.audit.action.kpmrPriceChange", "Price changed in KPMR"),
  "kpmr.model_delisted": () => t("format.audit.action.kpmrModelDelisted", "Model taken off KPMR"),
  "org.create": () => t("format.audit.action.orgCreate", "Organization created"),
  "org.update": () => t("format.audit.action.orgUpdate", "Organization changed"),
  "org.ownership_transfer": () => t("format.audit.action.orgOwnershipTransfer", "Ownership transferred"),
  "org.owner_appointed": () => t("format.audit.action.orgOwnerAppointed", "Owner added"),
  "org.owner_appointed_by_instance_admin": () => t("format.audit.action.orgOwnerAppointedByInstanceAdmin", "Owner added by an instance admin"),
  "org.schedule_deletion": () => t("format.audit.action.orgScheduleDeletion", "Deletion scheduled"),
  "org.restore": () => t("format.audit.action.orgRestore", "Organization restored"),
  "org.delete_now": () => t("format.audit.action.orgDeleteNow", "Organization deleted at once"),
  "org.delete_final": () => t("format.audit.action.orgDeleteFinal", "Organization deleted"),
  "platform_key.create": () => t("format.audit.action.platformKeyCreate", "Platform key created"),
  "platform_key.disable": () => t("format.audit.action.platformKeyDisable", "Platform key disabled"),
  "platform_key.enable": () => t("format.audit.action.platformKeyEnable", "Platform key enabled"),
  "platform_key.archive": () => t("format.audit.action.platformKeyArchive", "Platform key archived"),
  "platform_key.rotate": () => t("format.audit.action.platformKeyRotate", "Platform key rotated"),
  "project.create": () => t("format.audit.action.projectCreate", "Project created"),
  "project.update": () => t("format.audit.action.projectUpdate", "Project changed"),
  "project.default": () => t("format.audit.action.projectDefault", "Default project changed"),
  "project.key_policy": () => t("format.audit.action.projectKeyPolicy", "Project's key lifetime changed"),
  "response_cache.org_update": () => t("format.audit.action.responseCacheOrgUpdate", "Response cache allowed or forbidden"),
  "response_cache.project_update": () => t("format.audit.action.responseCacheProjectUpdate", "A project's response cache changed"),
  "response_cache.clear": () => t("format.audit.action.responseCacheClear", "A project's response cache cleared"),
  "project.archive": () => t("format.audit.action.projectArchive", "Project archived"),
  "project.member_add": () => t("format.audit.action.projectMemberAdd", "Project member added"),
  "project.member_rights_update": () => t("format.audit.action.projectMemberRightsUpdate", "Project member's role or rights changed"),
  "project.member_remove": () => t("format.audit.action.projectMemberRemove", "Project member removed"),
  "role.create": () => t("format.audit.action.roleCreate", "Role created"),
  "role.update": () => t("format.audit.action.roleUpdate", "Role changed"),
  "role.delete": () => t("format.audit.action.roleDelete", "Role deleted"),
  "security.sso_update": () => t("format.audit.action.securitySsoUpdate", "Single sign-on set up or changed"),
  "security.sso_delete": () => t("format.audit.action.securitySsoDelete", "Single sign-on disconnected"),
  "security.sso_domain_verify": () => t("format.audit.action.securitySsoDomainVerify", "Email domain verified"),
  "security.require_sso": () => t("format.audit.action.securityRequireSso", "Single sign-on requirement changed"),
  "security.require_2fa": () => t("format.audit.action.securityRequire2fa", "Two-factor requirement changed"),
  "security.key_policy": () => t("format.audit.action.securityKeyPolicy", "Key lifetime changed"),
  "security.agents_policy": () => t("format.audit.action.securityAgentsPolicy", "Organization agent policy changed"),
  "service_account.create": () => t("format.audit.action.serviceAccountCreate", "Service account created"),
  "service_account.update": () => t("format.audit.action.serviceAccountUpdate", "Service account changed"),
  "service_account.archive": () => t("format.audit.action.serviceAccountArchive", "Service account archived"),
  "user.signup": () => t("format.audit.action.userSignup", "Account created by sign-up"),
  "user.create_by_invite": () => t("format.audit.action.userCreateByInvite", "Account created from an invitation"),
  "user.create_by_sso": () => t("format.audit.action.userCreateBySso", "Account created by single sign-on"),
  "user.update": () => t("format.audit.action.userUpdate", "Profile changed"),
  "user.email_change_request": () => t("format.audit.action.userEmailChangeRequest", "Email change requested"),
  "user.email_change_cancel": () => t("format.audit.action.userEmailChangeCancel", "Email change canceled"),
  "user.email_change": () => t("format.audit.action.userEmailChange", "Email address changed"),
  "user.password_change": () => t("format.audit.action.userPasswordChange", "Password changed"),
  "user.password_reset": () => t("format.audit.action.userPasswordReset", "Password reset"),
  "user.totp_enable": () => t("format.audit.action.userTotpEnable", "Two-factor authentication turned on"),
  "user.totp_disable": () => t("format.audit.action.userTotpDisable", "Two-factor authentication turned off"),
  "user.recovery_codes_regenerate": () => t("format.audit.action.userRecoveryCodesRegenerate", "Recovery codes regenerated"),
  "user.2fa_reset": () => t("format.audit.action.user2faReset", "Two-factor authentication reset"),
  "user.sign_in_confirm": () => t("format.audit.action.userSignInConfirm", "Confirmed the sign-in through single sign-on"),
  "user.sign_in_link": () => t("format.audit.action.userSignInLink", "Sign-in link printed on the command line"),
  "user.sso_link": () => t("format.audit.action.userSsoLink", "Linked to single sign-on"),
  "user.sso_unlink": () => t("format.audit.action.userSsoUnlink", "Unlinked from single sign-on"),
  "user.sessions_revoke": () => t("format.audit.action.userSessionsRevoke", "Every session ended"),
  "user.notifications": () => t("format.audit.action.userNotifications", "Notification settings changed"),
  "user.deactivate": () => t("format.audit.action.userDeactivate", "User deactivated"),
  "user.reactivate": () => t("format.audit.action.userReactivate", "User reactivated"),
  "user.instance_admin": () => t("format.audit.action.userInstanceAdmin", "Instance admin rights changed"),
  "user.delete": () => t("format.audit.action.userDelete", "User deleted"),
};

export const AUDIT_ACTIONS: Record<string, string[]> = Object.keys(ACTION_LABELS).reduce<Record<string, string[]>>((kinds, action) => {
  const kind = action.slice(0, action.indexOf("."));
  (kinds[kind] ??= []).push(action);
  return kinds;
}, {});

// Events an older gateway wrote, before KPMR replaced models.dev; the audit log keeps them as they are.
const RETIRED_ACTION_LABELS: Record<string, () => string> = {
  "models_dev.sync": () => t("format.audit.action.modelsDevSync", "models.dev synced"),
  "models_dev.price_change": () => t("format.audit.action.modelsDevPriceChange", "Price changed on models.dev"),
  "models_dev.model_delisted": () => t("format.audit.action.modelsDevModelDelisted", "Model taken off models.dev"),
};

export const auditActionLabel = (action: string): string => (ACTION_LABELS[action] ?? RETIRED_ACTION_LABELS[action])?.() ?? action;

export function auditActionOptions(): { value: string; label: string; description: string }[] {
  return Object.entries(AUDIT_ACTIONS).flatMap(([kind, actions]) => [
    {
      value: `${kind}.`,
      label: t("format.audit.everyActionOf", "{kind}: every action", { kind: (KIND_LABELS[kind] as () => string)() }),
      description: `${kind}.*`,
    },
    ...actions.map((a) => ({ value: a, label: auditActionLabel(a), description: a })),
  ]);
}

const TARGET_KINDS: Record<string, () => string> = {
  agent: () => t("format.targetKind.agent", "Agent"),
  user: () => t("format.targetKind.user", "User"),
  org: () => t("common.organization", "Organization"),
  invite: () => t("format.targetKind.invite", "Invitation"),
  project: () => t("common.project", "Project"),
  service_account: () => t("format.targetKind.serviceAccount", "Service account"),
  limit: () => t("format.targetKind.limit", "Limit"),
  credential: () => t("format.targetKind.credential", "Provider key"),
  custom_provider: () => t("format.targetKind.customProvider", "Custom provider"),
  catalogue_model: () => t("common.model", "Model"),
  alias: () => t("format.targetKind.alias", "Alias"),
  platform_key: () => t("format.targetKind.platformKey", "Platform key"),
  key: () => t("format.targetKind.key", "Key"),
  role: () => t("format.targetKind.role", "Role"),
  update: () => t("format.targetKind.update", "Update"),
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

const text = (v: unknown): string | null => (typeof v === "string" && v ? v : null);

export function snapshotName(e: { target_kind?: string | null; before?: unknown; after?: unknown }): string | null {
  for (const side of [obj(e.after), obj(e.before)]) {
    if (!side) continue;
    const name =
      e.target_kind === "catalogue_model"
        ? text(side["provider"]) && text(side["model_id"])
          ? `${text(side["provider"])}/${text(side["model_id"])}`
          : text(side["model_id"])
        : e.target_kind === "invite"
          ? text(side["email"])
          : e.target_kind === "limit"
            ? text(side["description"])
            : e.target_kind === "org"
              ? (text(obj(side["org"])?.["name"]) ?? text(side["name"]))
              : (text(side["name"]) ?? (e.target_kind === "user" ? text(side["email"]) : null));
    if (name) return name;
  }
  return null;
}
