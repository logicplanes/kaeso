import { compareNames } from "~/format/collate";
import { t } from "~/i18n";
import type { KeySort } from "./key-sort";

export { isKeySort, type KeySort } from "./key-sort";

export const keySortOptions = () => [
  { value: "name", label: t("keys.sortName", "Name") },
  { value: "used", label: t("keys.sortUsed", "Last used") },
  { value: "expiry", label: t("keys.sortExpiry", "Expires soonest") },
];

type Listed = { name: string; redacted: string; last_used_at?: string | null | undefined; expires_at?: string | null | undefined };

const time = (v: string | null | undefined): number | null => (v ? Date.parse(v) : null);

// Newest use first and soonest expiry first; a key never used or never expiring comes last.
function byTime(a: number | null, b: number | null, newestFirst: boolean): number {
  if (a === b) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  return newestFirst ? b - a : a - b;
}

/** The keys whose name or redacted secret holds the query, in the chosen order. */
export function findKeys<K extends Listed>(keys: readonly K[], query: string, sort: KeySort | undefined): K[] {
  const words = query.trim().toLocaleLowerCase();
  const found = words ? keys.filter((k) => k.name.toLocaleLowerCase().includes(words) || k.redacted.toLocaleLowerCase().includes(words)) : [...keys];
  return found.sort(
    (a, b) =>
      (sort === "used" ? byTime(time(a.last_used_at), time(b.last_used_at), true) : 0) ||
      (sort === "expiry" ? byTime(time(a.expires_at), time(b.expires_at), false) : 0) ||
      compareNames(a.name, b.name),
  );
}
