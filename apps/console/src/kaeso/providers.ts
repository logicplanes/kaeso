import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import type { ProviderPickerOption } from "@logicplanes/kaeso-ui";
import type { ModelPickerProvider } from "@logicplanes/kaeso-ui/playground";
import type { CredentialView } from "~/api/generated/types";
import { orgCredentialsQuery } from "~/api/queries/credentials";
import { useOrgRights } from "~/auth/session";
import { PROVIDER_IDS, providerLabel } from "~/format/labels";
import { t } from "~/i18n";

export function providerOptions(keys: readonly CredentialView[], ids: readonly string[] = PROVIDER_IDS): ProviderPickerOption[] {
  const all = [...new Set([...ids, ...(ids === PROVIDER_IDS ? keys.map((k) => k.provider) : [])])];
  return all.map((id) => {
    const count = keys.filter((k) => k.provider === id).length;
    return {
      id,
      name: providerLabel(id),
      ...(count ? { connected: true, hint: t("kaeso.providerKey.count", "{count, plural, one {# key} other {# keys}}", { count }) } : {}),
    };
  });
}

export function useOrgProviderKeys(org: string): { keys: CredentialView[]; known: boolean } {
  const canView = useOrgRights(org).has("provider_credentials.view");
  const credentials = useQuery({ ...orgCredentialsQuery(org), enabled: canView });
  return { keys: credentials.data ?? [], known: canView && !!credentials.data };
}

export function useProviderOptions(org: string, ids?: readonly string[]): ProviderPickerOption[] {
  const { keys } = useOrgProviderKeys(org);
  return useMemo(() => providerOptions(keys, ids), [keys, ids]);
}

export function useModelPickerProviders(org: string, ids: readonly string[]): ModelPickerProvider[] {
  const { keys } = useOrgProviderKeys(org);
  return useMemo(() => ids.map((id) => ({ id, name: providerLabel(id), connected: keys.some((k) => k.provider === id) })), [keys, ids]);
}
