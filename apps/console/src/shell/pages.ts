import { icons, type IconData } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";
import type { LastPlace } from "./last";

export interface OrgAreaPage {
  id: string;
  label: () => string;
  icon: IconData;
  path: string;
  right: string;
}

export const ORG_AREA_PAGES: OrgAreaPage[] = [
  { id: "overview", label: () => t("shell.page.overview", "Overview"), icon: icons.overview, path: "", right: "org.view" },
  { id: "projects", label: () => t("common.projects", "Projects"), icon: icons.project, path: "projects", right: "org.view" },
  { id: "providers", label: () => t("shell.page.providers", "Providers"), icon: icons.provider, path: "providers", right: "provider_credentials.view" },
];

export const orgPath = (org: string) => `/orgs/${org}`;

export interface ProjectPage {
  id: string;
  label: () => string;
  icon: IconData;
  path: string;
  right: string;
  key?: string;
  shortcut?: string[][];
}

export const PROJECT_PAGES: ProjectPage[] = [
  {
    id: "overview",
    label: () => t("shell.page.overview", "Overview"),
    icon: icons.overview,
    path: "",
    right: "usage.view",
    key: "o",
    shortcut: [["g"], ["o"]],
  },
  { id: "playground", label: () => t("shell.page.playground", "Playground"), icon: icons.code, path: "playground", right: "playground.use" },
  { id: "keys", label: () => t("shell.page.apiKeys", "API keys"), icon: icons.key, path: "keys", right: "keys.all.view", key: "k", shortcut: [["g"], ["k"]] },
  {
    id: "logs",
    label: () => t("shell.page.requestLogs", "Request logs"),
    icon: icons.requestLog,
    path: "logs",
    right: "logs.metadata.view",
    key: "l",
    shortcut: [["g"], ["l"]],
  },
  { id: "models", label: () => t("common.models", "Models"), icon: icons.model, path: "models", right: "models.view", key: "m", shortcut: [["g"], ["m"]] },
  { id: "limits", label: () => t("shell.page.limits", "Limits"), icon: icons.limit, path: "limits", right: "limits.view" },
  { id: "members", label: () => t("common.members", "Members"), icon: icons.members, path: "members", right: "project.members.view" },
  {
    id: "service-accounts",
    label: () => t("shell.page.serviceAccounts", "Service accounts"),
    icon: icons.serviceAccount,
    path: "service-accounts",
    right: "service_accounts.view",
  },
  { id: "audit-log", label: () => t("shell.page.auditLog", "Audit log"), icon: icons.audit, path: "audit-log", right: "project.audit_log.view" },
  {
    id: "settings",
    label: () => t("common.settings", "Settings"),
    icon: icons.settings,
    path: "settings",
    right: "project.view",
    key: "p",
    shortcut: [["g"], ["p"]],
  },
];

export const projectPages = (requestLogs: boolean): ProjectPage[] => PROJECT_PAGES.filter((p) => requestLogs || p.id !== "logs");

export const projectPath = (org: string, project: string) => `/orgs/${org}/projects/${project}`;

export const placePath = (place: LastPlace) => (place.project ? projectPath(place.org, place.project) : orgPath(place.org));
