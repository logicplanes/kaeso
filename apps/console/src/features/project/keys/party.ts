import { useCallback } from "react";
import { useQuery } from "@tanstack/react-query";
import type { KeyState } from "@logicplanes/kaeso-ui";
import type { Party } from "~/api/generated/types";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { useSession } from "~/auth/session";
import { useProjectRights } from "~/auth/rights";
import { t } from "~/i18n";

export const asKeyState = (s: string): KeyState => (s === "disabled" || s === "expired" || s === "archived" ? s : "active");

export const yourName = (name: string) => t("keys.party.you", "{name} (you)", { name });
export const serviceAccountName = (name: string) => t("keys.party.serviceAccount", "{name} (service account)", { name });

export function usePartyName(org: string, project: string): (p: Party) => string {
  const session = useSession();
  const rights = useProjectRights(org, project);
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const accounts = useQuery({ ...serviceAccountsQuery(org, project, true), enabled: rights.has("service_accounts.view") });
  return useCallback(
    (p: Party) => {
      if (p.kind === "system") return t("keys.party.system", "System");
      if (p.kind === "user") {
        if (p.id === session.user.id) return yourName(session.user.name);
        return members.data?.find((m) => m.user.id === p.id)?.user.name ?? p.id ?? t("keys.party.aMember", "A member");
      }
      if (p.kind === "service_account") return accounts.data?.find((a) => a.id === p.id)?.name ?? p.id ?? t("keys.party.aServiceAccount", "A service account");
      if (p.kind === "org_platform_key")
        return p.id ? t("keys.party.platformKey", "Platform key {id}", { id: p.id }) : t("keys.party.aPlatformKey", "Platform key");
      if (p.kind === "api_key") return p.id ? t("keys.party.apiKey", "API key {id}", { id: p.id }) : t("keys.party.anApiKey", "API key");
      return p.id ?? p.kind;
    },
    [accounts.data, members.data, session.user.id, session.user.name],
  );
}

export function usePartyNamesLoading(org: string, project: string): boolean {
  const rights = useProjectRights(org, project);
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const accounts = useQuery({ ...serviceAccountsQuery(org, project, true), enabled: rights.has("service_accounts.view") });
  return members.isLoading || accounts.isLoading;
}
