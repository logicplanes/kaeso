import type { MyMembership } from "~/api/generated/types";
import { readLast } from "./last";

export function homeOrg(memberships: MyMembership[]): string | null {
  const last = readLast();
  if (last && memberships.some((m) => m.org_id === last.org)) return last.org;
  return memberships[0]?.org_id ?? null;
}
