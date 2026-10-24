import { t } from "~/i18n";
import english from "~/i18n/locales/en/rights.json";

const rights: Readonly<Record<string, () => [string, string]>> = {
  "org.view": () => [t("rights.orgView.name", "View organization"), t("rights.orgView.description", "See the organization, its settings and its roles.")],
  "org.update": () => [
    t("rights.orgUpdate.name", "Edit organization"),
    t("rights.orgUpdate.description", "Rename the organization, change its picture and general settings, and choose the default project."),
  ],
  "org.delete": () => [
    t("rights.orgDelete.name", "Delete organization"),
    t("rights.orgDelete.description", "Schedule the organization for deletion, or restore it while it waits."),
  ],
  "org.ownership.transfer": () => [
    t("rights.orgOwnershipTransfer.name", "Transfer ownership"),
    t("rights.orgOwnershipTransfer.description", "Make another member an owner in your place; only owners hold it."),
  ],
  "members.view": () => [t("rights.membersView.name", "View members"), t("rights.membersView.description", "See members, their roles and their rights.")],
  "members.invite": () => [
    t("rights.membersInvite.name", "Invite members"),
    t("rights.membersInvite.description", "Invite people by email with a role you could grant yourself."),
  ],
  "members.invites.revoke": () => [
    t("rights.membersInvitesRevoke.name", "Cancel invites"),
    t("rights.membersInvitesRevoke.description", "Cancel pending invitations."),
  ],
  "members.remove": () => [
    t("rights.membersRemove.name", "Remove members"),
    t("rights.membersRemove.description", "Remove members from the organization and archive their keys in it."),
  ],
  "members.rights.update": () => [
    t("rights.membersRightsUpdate.name", "Change member roles"),
    t("rights.membersRightsUpdate.description", "Change members' org roles and individual org rights."),
  ],
  "roles.manage": () => [
    t("rights.rolesManage.name", "Manage roles"),
    t("rights.rolesManage.description", "Create, edit and delete the organization's custom roles."),
  ],
  "projects.view.all": () => [
    t("rights.projectsViewAll.name", "View all projects"),
    t("rights.projectsViewAll.description", "See every project, including ones the member isn't in."),
  ],
  "projects.create": () => [
    t("rights.projectsCreate.name", "Create projects"),
    t("rights.projectsCreate.description", "Create projects; the creator becomes their admin."),
  ],
  "projects.archive.any": () => [
    t("rights.projectsArchiveAny.name", "Archive any project"),
    t("rights.projectsArchiveAny.description", "Archive any project and all its keys."),
  ],
  "provider_credentials.view": () => [
    t("rights.providerCredentialsView.name", "View provider keys"),
    t("rights.providerCredentialsView.description", "See the organization's provider keys (metadata only, never secrets)."),
  ],
  "provider_credentials.create": () => [
    t("rights.providerCredentialsCreate.name", "Add provider keys"),
    t("rights.providerCredentialsCreate.description", "Add provider keys to the organization."),
  ],
  "provider_credentials.replace": () => [
    t("rights.providerCredentialsReplace.name", "Change provider keys"),
    t("rights.providerCredentialsReplace.description", "Replace and rename provider keys, choose each provider's default and assign keys to projects."),
  ],
  "provider_credentials.delete": () => [
    t("rights.providerCredentialsDelete.name", "Delete provider keys"),
    t("rights.providerCredentialsDelete.description", "Remove a provider key from the organization."),
  ],
  "security.sso.manage": () => [
    t("rights.securitySsoManage.name", "Manage SSO"),
    t("rights.securitySsoManage.description", "Configure the organization's identity provider and the role of people who arrive through it."),
  ],
  "security.sso.require": () => [
    t("rights.securitySsoRequire.name", "Require SSO"),
    t("rights.securitySsoRequire.description", "Require the accounts this organization manages to sign in only through its SSO."),
  ],
  "security.2fa.require": () => [
    t("rights.security2faRequire.name", "Require two-factor"),
    t("rights.security2faRequire.description", "Require two-factor authentication for members."),
  ],
  "security.key_policy.manage": () => [
    t("rights.securityKeyPolicyManage.name", "Set key lifetime"),
    t("rights.securityKeyPolicyManage.description", "Set the organization's maximum key lifetime."),
  ],
  "platform_keys.view": () => [
    t("rights.platformKeysView.name", "View platform keys"),
    t("rights.platformKeysView.description", "See the organization's platform keys (metadata)."),
  ],
  "platform_keys.create": () => [
    t("rights.platformKeysCreate.name", "Create platform keys"),
    t("rights.platformKeysCreate.description", "Create and rotate organization platform keys, which manage everything at org level."),
  ],
  "platform_keys.revoke": () => [
    t("rights.platformKeysRevoke.name", "Disable platform keys"),
    t("rights.platformKeysRevoke.description", "Disable, enable or archive organization platform keys."),
  ],
  "logs.settings.manage": () => [
    t("rights.logsSettingsManage.name", "Manage log settings"),
    t("rights.logsSettingsManage.description", "Set log retention and switch content logging on or off for the organization."),
  ],
  "logs.storage.view": () => [
    t("rights.logsStorageView.name", "View log storage"),
    t("rights.logsStorageView.description", "See how many log entries exist and how much space they take."),
  ],
  "logs.delete": () => [t("rights.logsDelete.name", "Delete logs"), t("rights.logsDelete.description", "Delete the organization's logs for a time range.")],
  "usage.org.view": () => [t("rights.usageOrgView.name", "View org usage"), t("rights.usageOrgView.description", "See usage across all projects.")],
  "costs.org.view": () => [t("rights.costsOrgView.name", "View org costs"), t("rights.costsOrgView.description", "See costs across all projects.")],
  "billing.view": () => [
    t("rights.billingView.name", "View billing"),
    t("rights.billingView.description", "See plan, invoices and payment status (Kaeso Cloud)."),
  ],
  "billing.manage": () => [
    t("rights.billingManage.name", "Manage billing"),
    t("rights.billingManage.description", "Change the plan and payment method (Kaeso Cloud)."),
  ],
  "audit_log.view": () => [t("rights.auditLogView.name", "View audit log"), t("rights.auditLogView.description", "Read the organization's audit log.")],
  "audit_log.export": () => [
    t("rights.auditLogExport.name", "Export audit log"),
    t("rights.auditLogExport.description", "Export the organization's audit log."),
  ],
  "project.view": () => [t("rights.projectView.name", "View project"), t("rights.projectView.description", "See the project and its settings.")],
  "project.update": () => [
    t("rights.projectUpdate.name", "Edit project"),
    t("rights.projectUpdate.description", "Rename the project and change its settings and picture."),
  ],
  "project.archive": () => [
    t("rights.projectArchive.name", "Archive project"),
    t("rights.projectArchive.description", "Archive this project and all its keys."),
  ],
  "project.members.view": () => [
    t("rights.projectMembersView.name", "View members"),
    t("rights.projectMembersView.description", "See the project's members, their roles and their rights."),
  ],
  "project.members.add": () => [
    t("rights.projectMembersAdd.name", "Add members"),
    t("rights.projectMembersAdd.description", "Add organization members to the project with a role you could grant yourself."),
  ],
  "project.members.remove": () => [
    t("rights.projectMembersRemove.name", "Remove members"),
    t("rights.projectMembersRemove.description", "Remove members from the project and archive their keys in it."),
  ],
  "project.members.rights.update": () => [
    t("rights.projectMembersRightsUpdate.name", "Change member roles"),
    t("rights.projectMembersRightsUpdate.description", "Change members' project roles and individual project rights."),
  ],
  "service_accounts.view": () => [
    t("rights.serviceAccountsView.name", "View service accounts"),
    t("rights.serviceAccountsView.description", "See the project's service accounts and their rights."),
  ],
  "service_accounts.create": () => [
    t("rights.serviceAccountsCreate.name", "Create service accounts"),
    t("rights.serviceAccountsCreate.description", "Create service accounts with rights you hold yourself."),
  ],
  "service_accounts.rights.update": () => [
    t("rights.serviceAccountsRightsUpdate.name", "Change service accounts"),
    t("rights.serviceAccountsRightsUpdate.description", "Rename service accounts and change their roles and rights."),
  ],
  "service_accounts.archive": () => [
    t("rights.serviceAccountsArchive.name", "Archive service accounts"),
    t("rights.serviceAccountsArchive.description", "Archive service accounts and their keys."),
  ],
  "api.request": () => [
    t("rights.apiRequest.name", "Call models with keys"),
    t("rights.apiRequest.description", "Call the project's models with one's own API keys."),
  ],
  "playground.use": () => [
    t("rights.playgroundUse.name", "Use the Playground"),
    t("rights.playgroundUse.description", "Call the project's models from the console Playground."),
  ],
  "keys.own.create": () => [
    t("rights.keysOwnCreate.name", "Create own keys"),
    t("rights.keysOwnCreate.description", "Create and rotate API keys for oneself."),
  ],
  "keys.own.manage": () => [
    t("rights.keysOwnManage.name", "Manage own keys"),
    t("rights.keysOwnManage.description", "Disable, enable or archive one's own API keys."),
  ],
  "keys.all.view": () => [t("rights.keysAllView.name", "View all keys"), t("rights.keysAllView.description", "See every key in the project (metadata).")],
  "keys.all.manage": () => [
    t("rights.keysAllManage.name", "Manage all keys"),
    t("rights.keysAllManage.description", "Disable, enable or archive anyone's keys in the project."),
  ],
  "service_accounts.keys.create": () => [
    t("rights.serviceAccountsKeysCreate.name", "Create service account keys"),
    t("rights.serviceAccountsKeysCreate.description", "Create and rotate keys for the project's service accounts."),
  ],
  "keys.platform_access.grant": () => [
    t("rights.keysPlatformAccessGrant.name", "Give keys platform access"),
    t("rights.keysPlatformAccessGrant.description", "Give a new key access to the Platform API."),
  ],
  "security.key_policy.project.manage": () => [
    t("rights.securityKeyPolicyProjectManage.name", "Set key lifetime"),
    t("rights.securityKeyPolicyProjectManage.description", "Set the project's maximum key lifetime."),
  ],
  "models.view": () => [t("rights.modelsView.name", "View models"), t("rights.modelsView.description", "See the project's models, aliases and routing.")],
  "models.manage": () => [t("rights.modelsManage.name", "Manage models"), t("rights.modelsManage.description", "Add, change and remove models and aliases.")],
  "limits.view": () => [
    t("rights.limitsView.name", "View limits"),
    t("rights.limitsView.description", "See project, member and key limits, budgets and model access."),
  ],
  "limits.manage": () => [
    t("rights.limitsManage.name", "Manage limits"),
    t("rights.limitsManage.description", "Change project, member and key limits, budgets and model access."),
  ],
  "logs.metadata.view": () => [
    t("rights.logsMetadataView.name", "View request logs"),
    t("rights.logsMetadataView.description", "See request logs: model, tokens, cost, latency, status and key."),
  ],
  "logs.content.view": () => [
    t("rights.logsContentView.name", "View prompts and outputs"),
    t("rights.logsContentView.description", "See request and response bodies in the logs."),
  ],
  "logs.export": () => [t("rights.logsExport.name", "Export logs"), t("rights.logsExport.description", "Export the project's request logs.")],
  "logs.content.manage": () => [
    t("rights.logsContentManage.name", "Switch content logging"),
    t("rights.logsContentManage.description", "Switch content logging on or off for this project."),
  ],
  "usage.view": () => [t("rights.usageView.name", "View usage"), t("rights.usageView.description", "See the project's usage.")],
  "costs.view": () => [t("rights.costsView.name", "View costs"), t("rights.costsView.description", "See the project's costs.")],
  "project.audit_log.view": () => [
    t("rights.projectAuditLogView.name", "View project audit log"),
    t("rights.projectAuditLogView.description", "Read the audit log of changes in this project."),
  ],
};

const categories: Readonly<Record<string, [string, () => string]>> = {
  Organization: ["organization", () => t("rights.category.organization", "Organization")],
  "Members & access": ["members", () => t("rights.category.members", "Members & access")],
  Projects: ["projects", () => t("rights.category.projects", "Projects")],
  Providers: ["providers", () => t("rights.category.providers", "Providers")],
  Security: ["security", () => t("rights.category.security", "Security")],
  Keys: ["keys", () => t("rights.category.keys", "Keys")],
  "Logs & data": ["logs", () => t("rights.category.logs", "Logs & data")],
  "Usage & billing": ["usage", () => t("rights.category.usageBilling", "Usage & billing")],
  Audit: ["audit", () => t("rights.category.audit", "Audit")],
  Project: ["project", () => t("rights.category.project", "Project")],
  "Model use": ["model_use", () => t("rights.category.modelUse", "Model use")],
  Models: ["models", () => t("rights.category.models", "Models")],
  "Limits & budgets": ["limits", () => t("rights.category.limits", "Limits & budgets")],
  "Usage & costs": ["usage", () => t("rights.category.usageCosts", "Usage & costs")],
};

const roleNames: Readonly<Record<string, [string, () => string]>> = {
  owner: ["Owner", () => t("rights.role.owner.name", "Owner")],
  admin: ["Admin", () => t("rights.role.admin.name", "Admin")],
  billing: ["Billing", () => t("rights.role.billing.name", "Billing")],
  developer: ["Developer", () => t("rights.role.developer.name", "Developer")],
  auditor: ["Auditor", () => t("rights.role.auditor.name", "Auditor")],
  member: ["Member", () => t("rights.role.member.name", "Member")],
  viewer: ["Viewer", () => t("rights.role.viewer.name", "Viewer")],
};

const roleDescriptions: Readonly<Record<string, [string, () => string]>> = {
  "org.owner": [
    "Holds every right, now and in future releases; only owners manage owners.",
    () => t("rights.role.org.owner.description", "Holds every right, now and in future releases; only owners manage owners."),
  ],
  "org.admin": [
    "Manages the organization and every project, except deleting it, transferring ownership and changing billing.",
    () =>
      t("rights.role.org.admin.description", "Manages the organization and every project, except deleting it, transferring ownership and changing billing."),
  ],
  "org.billing": [
    "Manages billing and sees usage and costs across all projects.",
    () => t("rights.role.org.billing.description", "Manages billing and sees usage and costs across all projects."),
  ],
  "org.developer": [
    "Creates projects and sees usage, costs and provider keys; works in the projects they are added to.",
    () => t("rights.role.org.developer.description", "Creates projects and sees usage, costs and provider keys; works in the projects they are added to."),
  ],
  "org.auditor": [
    "Reads the whole organization for reviews, including the audit log and every project, and changes nothing; never sees prompts or outputs.",
    () =>
      t(
        "rights.role.org.auditor.description",
        "Reads the whole organization for reviews, including the audit log and every project, and changes nothing; never sees prompts or outputs.",
      ),
  ],
  "org.member": [
    "Sees the organization and its members; works in the projects they are added to.",
    () => t("rights.role.org.member.description", "Sees the organization and its members; works in the projects they are added to."),
  ],
  "project.admin": [
    "Manages the project: members, keys, service accounts, models, limits, logs and settings.",
    () => t("rights.role.project.admin.description", "Manages the project: members, keys, service accounts, models, limits, logs and settings."),
  ],
  "project.developer": [
    "Calls models with their own keys and in the Playground, and reads the project, including prompts and outputs.",
    () =>
      t(
        "rights.role.project.developer.description",
        "Calls models with their own keys and in the Playground, and reads the project, including prompts and outputs.",
      ),
  ],
  "project.viewer": [
    "Reads the project's settings, models, limits, keys, usage and log metadata; changes nothing.",
    () => t("rights.role.project.viewer.description", "Reads the project's settings, models, limits, keys, usage and log metadata; changes nothing."),
  ],
};

const catalog: Readonly<Record<string, string>> = english;
const camel = (id: string) => id.replace(/[._]([a-z0-9])/g, (_, c: string) => c.toUpperCase());

export function rightEnglish(id: string): [string | undefined, string | undefined] {
  const base = `rights.${camel(id)}`;
  return [catalog[`${base}.name`], catalog[`${base}.description`]];
}

type Node = Record<string, unknown>;

function translateNode(o: Node): boolean {
  let changed = false;
  const set = (field: string, text: string) => {
    if (o[field] !== text) {
      o[field] = text;
      changed = true;
    }
  };
  const id = typeof o.id === "string" ? o.id : undefined;
  const name = typeof o.name === "string" ? o.name : undefined;
  if (id === undefined || name === undefined) return false;
  if (Array.isArray(o.needs) && rights[id]) {
    const [englishName, englishDescription] = rightEnglish(id);
    const [translatedName, translatedDescription] = rights[id]();
    if (name === englishName) set("name", translatedName);
    if (typeof o.description === "string" && o.description === englishDescription) set("description", translatedDescription);
  } else if (Array.isArray(o.rights) && o.kind === undefined) {
    const category = categories[name];
    if (category && category[0] === id) set("name", category[1]());
  } else if (o.kind === "builtin") {
    const role = roleNames[id];
    if (role && role[0] === name) set("name", role[1]());
    const description = typeof o.scope === "string" ? roleDescriptions[`${o.scope}.${id}`] : undefined;
    if (description && description[0] === o.description) set("description", description[1]());
  } else if (o.kind === "custom" && id === "deleted" && name === "Deleted role") {
    set("name", t("rights.role.deleted.name", "Deleted role"));
  } else if (name === "Deleted user") {
    set("name", t("rights.deletedUser", "Deleted user"));
  }
  return changed;
}

export async function translateBody(body: unknown): Promise<boolean> {
  let changed = false;
  const visit = (node: unknown) => {
    if (Array.isArray(node)) {
      for (const item of node) visit(item);
      return;
    }
    if (node === null || typeof node !== "object") return;
    if (translateNode(node as Node)) changed = true;
    for (const value of Object.values(node)) if (value !== null && typeof value === "object") visit(value);
  };
  visit(body);
  if (body !== null && typeof body === "object" && !Array.isArray(body)) {
    const top = body as Node;
    if (typeof top.message === "string" && typeof top.status === "string") {
      const { translateMessage } = await import("./gateway-errors");
      const message = translateMessage(top.message);
      if (message !== top.message) {
        top.message = message;
        changed = true;
      }
    }
  }
  return changed;
}

export const knownRights = (): string[] => Object.keys(rights);
