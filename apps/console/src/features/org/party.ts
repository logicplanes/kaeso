import { useCallback } from "react";
import { useQuery } from "@tanstack/react-query";
import { orgMembersQuery, platformKeysQuery } from "~/api/queries/orgs";
import { useOrgRights, useSession } from "~/auth/session";
import { t } from "~/i18n";
import { partyText, userLabel, type PartyLabel, type PartyRef } from "~/kaeso/party";

export function useOrgPartyLabel(org: string): (p: PartyRef) => PartyLabel {
  const session = useSession();
  const rights = useOrgRights(org);
  const members = useQuery({ ...orgMembersQuery(org), enabled: rights.has("members.view") });
  const keys = useQuery({ ...platformKeysQuery(org, true), enabled: rights.has("platform_keys.view") });
  return useCallback(
    (p: PartyRef) => {
      const named = (name: string): PartyLabel => ({ name, you: false });
      if (p.kind === "system") return named(t("org.party.system", "System"));
      if (p.kind === "user") {
        if (p.id === session.user.id) return userLabel(session.user, true);
        const member = members.data?.find((m) => m.user.id === p.id)?.user;
        return member ? userLabel(member) : named(p.id ?? t("org.party.member", "A member"));
      }
      if (p.kind === "org_platform_key") {
        const name = keys.data?.find((k) => k.id === p.id)?.name ?? p.id;
        return named(name ? t("org.party.platformKey", "Platform key {name}", { name }) : t("org.party.platformKeyUnnamed", "Platform key"));
      }
      if (p.kind === "api_key") return named(p.id ? t("org.party.apiKey", "API key {id}", { id: p.id }) : t("org.party.apiKeyUnnamed", "API key"));
      if (p.kind === "service_account")
        return named(p.id ? t("org.party.serviceAccount", "Service account {id}", { id: p.id }) : t("org.party.serviceAccountUnnamed", "Service account"));
      return named(p.id ?? p.kind);
    },
    [keys.data, members.data, session.user],
  );
}

export function useOrgPartyName(org: string): (p: PartyRef) => string {
  const label = useOrgPartyLabel(org);
  return useCallback((p: PartyRef) => partyText(label(p)), [label]);
}
