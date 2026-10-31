import { t } from "~/i18n";
import { rightEnglish, type NodeTranslator } from "./vocabulary";

const rights: Readonly<Record<string, () => [string, string]>> = {
  "instance.system.view": () => [
    t("rights.instanceSystemView.name", "View the system"),
    t("rights.instanceSystemView.description", "See the system and its model data, the instance settings and the update status."),
  ],
  "instance.settings.manage": () => [
    t("rights.instanceSettingsManage.name", "Change instance settings"),
    t("rights.instanceSettingsManage.description", "Change general, mail, logging, time zone and account settings, and send a test email."),
  ],
  "instance.updates.manage": () => [
    t("rights.instanceUpdatesManage.name", "Manage updates"),
    t("rights.instanceUpdatesManage.description", "Check for updates and change the update settings."),
  ],
  "instance.updates.apply": () => [
    t("rights.instanceUpdatesApply.name", "Apply updates"),
    t("rights.instanceUpdatesApply.description", "Install a new version of Kaeso."),
  ],
  "instance.users.view": () => [
    t("rights.instanceUsersView.name", "View users"),
    t("rights.instanceUsersView.description", "See every account of the instance and its organizations."),
  ],
  "instance.users.manage": () => [
    t("rights.instanceUsersManage.name", "Manage users"),
    t("rights.instanceUsersManage.description", "Invite, deactivate and reactivate accounts, end their sessions and reset their two-factor."),
  ],
  "instance.orgs.view": () => [
    t("rights.instanceOrgsView.name", "View organizations"),
    t("rights.instanceOrgsView.description", "See every organization with its members, projects and usage."),
  ],
  "instance.orgs.manage": () => [
    t("rights.instanceOrgsManage.name", "Manage organizations"),
    t("rights.instanceOrgsManage.description", "Create and rename organizations, appoint owners and schedule or cancel their deletion."),
  ],
  "instance.audit_log.view": () => [
    t("rights.instanceAuditLogView.name", "View instance audit log"),
    t("rights.instanceAuditLogView.description", "Read the instance audit log."),
  ],
  "account.view": () => [
    t("rights.accountView.name", "View the account"),
    t("rights.accountView.description", "See the profile, organizations, notification settings and what's new."),
  ],
  "account.update": () => [
    t("rights.accountUpdate.name", "Edit the profile"),
    t("rights.accountUpdate.description", "Change the name, picture, language and appearance."),
  ],
  "account.keys.view": () => [
    t("rights.accountKeysView.name", "View personal keys"),
    t("rights.accountKeysView.description", "See the account's API keys in every project (metadata)."),
  ],
  "account.keys.manage": () => [
    t("rights.accountKeysManage.name", "Manage personal keys"),
    t("rights.accountKeysManage.description", "Disable, enable or archive the account's API keys."),
  ],
  "account.notifications.manage": () => [
    t("rights.accountNotificationsManage.name", "Change notifications"),
    t("rights.accountNotificationsManage.description", "Choose which emails the account gets."),
  ],
  "account.memberships.leave": () => [
    t("rights.accountMembershipsLeave.name", "Leave organizations"),
    t("rights.accountMembershipsLeave.description", "Leave organizations the account belongs to."),
  ],
};

const categories: Readonly<Record<string, [string, () => string]>> = {
  Instance: ["instance", () => t("rights.category.instance", "Instance")],
  Account: ["account", () => t("rights.category.account", "Account")],
};

const agentPresets: Readonly<Record<string, [string, () => string, string, () => string]>> = {
  observer: [
    "Observer",
    () => t("rights.agentPreset.observer.name", "Observer"),
    "Reads usage, costs, models, limits, log metadata and audit logs; never prompts or outputs.",
    () => t("rights.agentPreset.observer.description", "Reads usage, costs, models, limits, log metadata and audit logs; never prompts or outputs."),
  ],
  builder: [
    "Builder",
    () => t("rights.agentPreset.builder.name", "Builder"),
    "Works in projects like a developer and manages their models; sees personal keys.",
    () => t("rights.agentPreset.builder.description", "Works in projects like a developer and manages their models; sees personal keys."),
  ],
  operator: [
    "Operator",
    () => t("rights.agentPreset.operator.name", "Operator"),
    "Administers organizations and projects, except deleting, ownership and billing.",
    () => t("rights.agentPreset.operator.description", "Administers organizations and projects, except deleting, ownership and billing."),
  ],
  full: [
    "Full access",
    () => t("rights.agentPreset.full.name", "Full access"),
    "Everything you can do that an agent may be given.",
    () => t("rights.agentPreset.full.description", "Everything you can do that an agent may be given."),
  ],
};

const instancePresets: Readonly<Record<string, [string, () => string, string, () => string]>> = {
  none: [
    "None",
    () => t("rights.instancePreset.none.name", "None"),
    "No access to the instance.",
    () => t("rights.instancePreset.none.description", "No access to the instance."),
  ],
  read: [
    "Read",
    () => t("rights.instancePreset.read.name", "Read"),
    "Reads the system, users, organizations and the instance audit log.",
    () => t("rights.instancePreset.read.description", "Reads the system, users, organizations and the instance audit log."),
  ],
  configure: [
    "Configure",
    () => t("rights.instancePreset.configure.name", "Configure"),
    "Reads the instance and changes its settings and update settings.",
    () => t("rights.instancePreset.configure.description", "Reads the instance and changes its settings and update settings."),
  ],
  admin: [
    "Admin",
    () => t("rights.instancePreset.admin.name", "Admin"),
    "Everything instance admins do that an agent may be given.",
    () => t("rights.instancePreset.admin.description", "Everything instance admins do that an agent may be given."),
  ],
};

export const agentRightIds = (): string[] => Object.keys(rights);

export const translateAgentNode: NodeTranslator = (o, set, id, name) => {
  const preset = Array.isArray(o.org_ceiling)
    ? agentPresets[id]
    : Array.isArray(o.rights) && typeof o.description === "string" && o.kind === undefined
      ? instancePresets[id]
      : undefined;
  if (preset) {
    if (name === preset[0]) set("name", preset[1]());
    if (o.description === preset[2]) set("description", preset[3]());
    return true;
  }
  const right = Array.isArray(o.needs) ? rights[id] : undefined;
  if (right) {
    const [englishName, englishDescription] = rightEnglish(id);
    const [translatedName, translatedDescription] = right();
    if (name === englishName) set("name", translatedName);
    if (o.description === englishDescription) set("description", translatedDescription);
    return true;
  }
  const category = Array.isArray(o.rights) ? categories[name] : undefined;
  if (category?.[0] === id) {
    set("name", category[1]());
    return true;
  }
  return false;
};
