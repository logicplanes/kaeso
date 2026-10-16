import type { MyMembership } from "~/api/generated/types";
import { readLast, type LastPlace } from "./last";

export function homePlace(memberships: MyMembership[]): LastPlace | null {
  const last = readLast();
  const org = last ? memberships.find((m) => m.org_id === last.org) : undefined;
  if (last && org) return last.project && org.projects.some((p) => p.id === last.project) ? last : { org: last.org };
  const first = memberships[0];
  return first ? { org: first.org_id } : null;
}

export function defaultProjectOf(memberships: MyMembership[], org: string): string | null {
  const m = memberships.find((x) => x.org_id === org);
  return (m?.projects.find((p) => p.is_default) ?? m?.projects[0])?.id ?? null;
}
