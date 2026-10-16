import { icons, type IconData } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export interface SettingsPage {
  id: string;
  label: () => string;
  icon: IconData;
  path: string;
  right?: string;
}

export const ACCOUNT_PAGES: SettingsPage[] = [
  { id: "profile", label: () => t("shell.settings.profile", "Profile"), icon: icons.member, path: "profile" },
  { id: "security", label: () => t("shell.settings.security", "Security"), icon: icons.security, path: "security" },
  { id: "sessions", label: () => t("shell.settings.sessions", "Sessions"), icon: icons.session, path: "sessions" },
  { id: "keys", label: () => t("shell.page.apiKeys", "API keys"), icon: icons.key, path: "keys" },
  { id: "memberships", label: () => t("shell.settings.memberships", "Memberships"), icon: icons.organization, path: "memberships" },
  { id: "notifications", label: () => t("shell.settings.notifications", "Notifications"), icon: icons.notification, path: "notifications" },
  { id: "appearance", label: () => t("shell.settings.appearance", "Appearance"), icon: icons.themeSystem, path: "appearance" },
];
export const ORG_SETTINGS_PAGES: SettingsPage[] = [
  { id: "general", label: () => t("shell.settings.general", "General"), icon: icons.settings, path: "general", right: "org.view" },
  { id: "members", label: () => t("common.members", "Members"), icon: icons.members, path: "members", right: "members.view" },
  { id: "security", label: () => t("shell.settings.security", "Security"), icon: icons.security, path: "security", right: "org.view" },
  { id: "platform-keys", label: () => t("shell.settings.platformKeys", "Platform keys"), icon: icons.key, path: "platform-keys", right: "platform_keys.view" },
  { id: "logging", label: () => t("shell.settings.logging", "Logging"), icon: icons.requestLog, path: "logging", right: "org.view" },
  { id: "audit-log", label: () => t("shell.page.auditLog", "Audit log"), icon: icons.audit, path: "audit-log", right: "audit_log.view" },
];
export const INSTANCE_PAGES: SettingsPage[] = [
  { id: "users", label: () => t("shell.settings.users", "Users"), icon: icons.members, path: "users" },
  { id: "orgs", label: () => t("shell.organizations", "Organizations"), icon: icons.organization, path: "orgs" },
  { id: "settings", label: () => t("common.settings", "Settings"), icon: icons.settings, path: "settings" },
  { id: "model-sources", label: () => t("shell.settings.modelSources", "Model sources"), icon: icons.model, path: "model-sources" },
  { id: "system", label: () => t("shell.settings.system", "System"), icon: icons.gateway, path: "system" },
  { id: "audit-log", label: () => t("shell.page.auditLog", "Audit log"), icon: icons.audit, path: "audit-log" },
];
