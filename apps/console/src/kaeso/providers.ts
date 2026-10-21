import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import type { ProviderPickerOption } from "@logicplanes/kaeso-ui";
import type { ModelPickerProvider } from "@logicplanes/kaeso-ui/playground";
import type { CredentialView, ProviderView } from "~/api/generated/types";
import { orgCredentialsQuery } from "~/api/queries/credentials";
import { providersQuery } from "~/api/queries/providers";
import { useOrgRights } from "~/auth/session";
import { DIALECT_IDS } from "~/format/labels";
import { t } from "~/i18n";

export interface Providers {
  readonly list: readonly ProviderView[];
  get(id: string): ProviderView | undefined;
  name(id: string): string;
  dialects(id: string): readonly string[];
  byName(a: string, b: string): number;
}

export function providerCatalog(list: readonly ProviderView[]): Providers {
  const byId = new Map(list.map((p) => [p.id, p]));
  const name = (id: string) => byId.get(id)?.name ?? id.charAt(0).toUpperCase() + id.slice(1);
  return {
    list,
    get: (id) => byId.get(id),
    name,
    dialects: (id) => byId.get(id)?.dialects ?? DIALECT_IDS,
    byName: (a, b) => name(a).localeCompare(name(b)),
  };
}

export const keyTitle = (k: CredentialView, providers: Providers) =>
  t("kaeso.providerKey.title", "{provider} · {name}", { provider: providers.name(k.provider), name: k.name });

export function useProviders(org: string): Providers {
  const providers = useQuery(providersQuery(org));
  return useMemo(() => providerCatalog(providers.data ?? []), [providers.data]);
}

export function providerOptions(
  providers: Providers,
  keys: readonly CredentialView[],
  ids: readonly string[] = providers.list.map((p) => p.id),
): ProviderPickerOption[] {
  return ids.map((id) => {
    const count = keys.filter((k) => k.provider === id).length;
    return {
      id,
      name: providers.name(id),
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
  const providers = useProviders(org);
  const { keys } = useOrgProviderKeys(org);
  return useMemo(() => providerOptions(providers, keys, ids), [providers, keys, ids]);
}

export function useProviderFilterOptions(org: string): ProviderPickerOption[] {
  const providers = useProviders(org);
  const { keys } = useOrgProviderKeys(org);
  return useMemo(() => providerOptions(providers, keys, [...new Set([...providers.list.map((p) => p.id), ...keys.map((k) => k.provider)])]), [providers, keys]);
}

export function useModelPickerProviders(org: string, ids: readonly string[]): ModelPickerProvider[] {
  const providers = useProviders(org);
  const { keys } = useOrgProviderKeys(org);
  return useMemo(() => ids.map((id) => ({ id, name: providers.name(id), connected: keys.some((k) => k.provider === id) })), [providers, keys, ids]);
}
