import { useCallback } from "react";
import { useQuery } from "@tanstack/react-query";
import { orgMembersQuery, platformKeysQuery } from "~/api/queries/orgs";
import { useOrgRights, useSession } from "~/auth/session";
import { t } from "~/i18n";

export function useOrgPartyName(org: string): (p: { kind: string; id?: string | null | undefined }) => string {
  const session = useSession();
  const rights = useOrgRights(org);
  const members = useQuery({ ...orgMembersQuery(org), enabled: rights.has("members.view") });
  const keys = useQuery({ ...platformKeysQuery(org, true), enabled: rights.has("platform_keys.view") });
  return useCallback(
    (p) => {
      if (p.kind === "system") return t("org.party.system", "System");
      if (p.kind === "user") {
        if (p.id === session.user.id) return t("org.nameYou", "{name} (you)", { name: session.user.name });
        return members.data?.find((m) => m.user.id === p.id)?.user.name ?? p.id ?? t("org.party.member", "A member");
      }
      if (p.kind === "org_platform_key") {
        const name = keys.data?.find((k) => k.id === p.id)?.name ?? p.id;
        return name ? t("org.party.platformKey", "Platform key {name}", { name }) : t("org.party.platformKeyUnnamed", "Platform key");
      }
      if (p.kind === "api_key") return p.id ? t("org.party.apiKey", "API key {id}", { id: p.id }) : t("org.party.apiKeyUnnamed", "API key");
      if (p.kind === "service_account")
        return p.id ? t("org.party.serviceAccount", "Service account {id}", { id: p.id }) : t("org.party.serviceAccountUnnamed", "Service account");
      return p.id ?? p.kind;
    },
    [keys.data, members.data, session.user.id, session.user.name],
  );
}
