import { useCallback } from "react";
import { useQuery } from "@tanstack/react-query";
import type { KeyState } from "@logicplanes/kaeso-ui";
import type { KeyView } from "~/api/generated/types";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { useSession } from "~/auth/session";
import { useProjectRights } from "~/auth/rights";
import { t } from "~/i18n";
import { partyText, userLabel, type PartyLabel, type PartyRef } from "~/kaeso/party";

export const asKeyState = (s: string): KeyState => (s === "disabled" || s === "expired" || s === "archived" ? s : "active");

export const keyLabel = (k: Pick<KeyView, "name" | "replaced_by">): string =>
  k.replaced_by ? t("keys.replacedName", "{name} (replaced)", { name: k.name }) : k.name;

export const serviceAccountName = (name: string) => t("keys.party.serviceAccount", "{name} (service account)", { name });

export function usePartyLabel(org: string, project: string): (p: PartyRef) => PartyLabel {
  const session = useSession();
  const rights = useProjectRights(org, project);
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const accounts = useQuery({ ...serviceAccountsQuery(org, project, true), enabled: rights.has("service_accounts.view") });
  return useCallback(
    (p: PartyRef) => {
      const named = (name: string): PartyLabel => ({ name, you: false });
      if (p.kind === "system") return named(t("keys.party.system", "System"));
      if (p.kind === "user") {
        if (p.id === session.user.id) return userLabel(session.user, true);
        const member = members.data?.find((m) => m.user.id === p.id)?.user;
        return member ? userLabel(member) : named(p.id ?? t("keys.party.aMember", "A member"));
      }
      if (p.kind === "service_account")
        return named(accounts.data?.find((a) => a.id === p.id)?.name ?? p.id ?? t("keys.party.aServiceAccount", "A service account"));
      if (p.kind === "org_platform_key")
        return named(p.id ? t("keys.party.platformKey", "Platform key {id}", { id: p.id }) : t("keys.party.aPlatformKey", "Platform key"));
      if (p.kind === "api_key") return named(p.id ? t("keys.party.apiKey", "API key {id}", { id: p.id }) : t("keys.party.anApiKey", "API key"));
      return named(p.id ?? p.kind);
    },
    [accounts.data, members.data, session.user],
  );
}

export function usePartyName(org: string, project: string): (p: PartyRef) => string {
  const label = usePartyLabel(org, project);
  return useCallback((p: PartyRef) => partyText(label(p)), [label]);
}

export function usePartyNamesLoading(org: string, project: string): boolean {
  const rights = useProjectRights(org, project);
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const accounts = useQuery({ ...serviceAccountsQuery(org, project, true), enabled: rights.has("service_accounts.view") });
  return members.isLoading || accounts.isLoading;
}
