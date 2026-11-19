import { createFileRoute } from "@tanstack/react-router";
import { AccountKeysPage, type AccountKeysSearch } from "~/features/account/keys-page";
import { isKeySort } from "~/kaeso/key-sort";

const keysSearch = (s: Record<string, unknown>): AccountKeysSearch => (isKeySort(s.sort) ? { sort: s.sort } : {});

export const Route = createFileRoute("/_app/settings/account/keys")({ validateSearch: keysSearch, component: Keys });

function Keys() {
  return <AccountKeysPage search={Route.useSearch()} />;
}
