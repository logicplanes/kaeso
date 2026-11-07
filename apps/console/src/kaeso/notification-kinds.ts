import type { NotificationDelivery } from "~/api/generated/types";
import { t } from "~/i18n";

type Words = () => { name: string; description: string };

const kinds: Readonly<Record<string, Words>> = {
  account_security: () => ({
    name: t("account.notify.accountSecurity", "Account security"),
    description: t("account.notify.accountSecurityHint", "When your password, 2FA, address or sessions change."),
  }),
  new_app: () => ({
    name: t("account.notify.newApp", "A new app can act as you"),
    description: t("account.notify.newAppHint", "When you let an app or MCP client act as you."),
  }),
  export_ready: () => ({
    name: t("account.notify.exportReady", "Export finished"),
    description: t("account.notify.exportReadyHint", "When a log or audit log export you started finishes."),
  }),
  weekly_summary: () => ({
    name: t("account.notify.weeklySummary", "Weekly summary"),
    description: t("account.notify.weeklySummaryHint", "Mondays: requests, cost, tool calls and top models."),
  }),
  access_changed: () => ({
    name: t("account.notify.accessChanged", "Your access changed"),
    description: t("account.notify.accessChangedHint", "When you're added, removed or your rights change."),
  }),
  budget_alerts: () => ({
    name: t("account.notify.budgetAlerts", "Budget alerts"),
    description: t("account.notify.budgetAlertsHint", "When a budget you're responsible for crosses a threshold."),
  }),
  limit_reached: () => ({
    name: t("account.notify.limitReached", "Limit reached"),
    description: t("account.notify.limitReachedHint", "When a limit starts blocking requests."),
  }),
  key_expiry: () => ({
    name: t("account.notify.keyExpiry", "Your key expires"),
    description: t("account.notify.keyExpiryHint", "Before a key you own expires."),
  }),
  key_changed: () => ({
    name: t("account.notify.keyChanged", "Your key changed"),
    description: t("account.notify.keyChangedHint", "When someone else disables, rotates or archives it."),
  }),
  content_logging: () => ({
    name: t("account.notify.contentLogging", "Content logging changed"),
    description: t("account.notify.contentLoggingHint", "When a project starts or stops keeping prompts."),
  }),
  mcp_servers: () => ({
    name: t("account.notify.mcpServers", "MCP servers need attention"),
    description: t("account.notify.mcpServersHint", "When a connection signs out or a tool waits for review."),
  }),
  connection_expired: () => ({
    name: t("account.notify.connectionExpired", "Your connection expired"),
    description: t("account.notify.connectionExpiredHint", "When a server signs your own connection out."),
  }),
  portal_clients: () => ({
    name: t("account.notify.portalClients", "New client on a portal"),
    description: t("account.notify.portalClientsHint", "When someone connects a new client to a portal."),
  }),
  org_security: () => ({
    name: t("account.notify.orgSecurity", "Security settings changed"),
    description: t("account.notify.orgSecurityHint", "When someone changes sign-in rules, SSO or platform keys."),
  }),
  org_ownership: () => ({
    name: t("account.notify.orgOwnership", "Ownership and deletion"),
    description: t("account.notify.orgOwnershipHint", "When an organization changes owners or is deleted."),
  }),
  org_members: () => ({
    name: t("account.notify.orgMembers", "Member changes"),
    description: t("account.notify.orgMembersHint", "When people join or leave an organization."),
  }),
  invite_accepted: () => ({
    name: t("account.notify.inviteAccepted", "Your invitation was accepted"),
    description: t("account.notify.inviteAcceptedHint", "When someone you invited joins."),
  }),
  provider_keys: () => ({
    name: t("account.notify.providerKeys", "Provider keys need attention"),
    description: t("account.notify.providerKeysHint", "When a provider keeps refusing a key or an account."),
  }),
  model_changes: () => ({
    name: t("account.notify.modelChanges", "Model changes"),
    description: t("account.notify.modelChangesHint", "When a model in use is delisted or changes its price."),
  }),
  cost_report: () => ({
    name: t("account.notify.costReport", "Monthly cost report"),
    description: t("account.notify.costReportHint", "On the 1st: last month's cost by project and model."),
  }),
  instance_updates: () => ({
    name: t("account.notify.instanceUpdates", "Updates"),
    description: t("account.notify.instanceUpdatesHint", "When a new version is out or an update ends."),
  }),
  instance_people: () => ({
    name: t("account.notify.instancePeople", "New organizations and people"),
    description: t("account.notify.instancePeopleHint", "When someone signs up or creates an organization."),
  }),
  instance_settings: () => ({
    name: t("account.notify.instanceSettings", "Instance settings changed"),
    description: t("account.notify.instanceSettingsHint", "When another admin changes the instance's settings."),
  }),
};

const areas: Readonly<Record<string, () => string>> = {
  security: () => t("account.notifyArea.security", "Security"),
  personal: () => t("account.notifyArea.personal", "Personal"),
  project: () => t("account.notifyArea.project", "Project"),
  tools: () => t("account.notifyArea.tools", "Tools"),
  organization: () => t("account.notifyArea.organization", "Organization"),
  instance: () => t("account.notifyArea.instance", "Instance"),
};

export function kindWords(kind: { kind: string; name: string; description: string }): { name: string; description: string } {
  return kinds[kind.kind]?.() ?? { name: kind.name, description: kind.description };
}

export function kindName(kind: string, english: string): string {
  return kinds[kind]?.().name ?? english;
}

export function areaName(area: { area: string; name: string }): string {
  return areas[area.area]?.() ?? area.name;
}

const everyDelivery: NotificationDelivery[] = ["now", "daily", "off"];

export const deliveryOptions = (offered: readonly NotificationDelivery[] = everyDelivery): { value: NotificationDelivery; label: string }[] =>
  (
    [
      { value: "now", label: t("account.delivery.now", "Now") },
      { value: "daily", label: t("account.delivery.daily", "Daily") },
      { value: "off", label: t("account.delivery.off", "Off") },
    ] as const
  ).filter((o) => offered.includes(o.value));
