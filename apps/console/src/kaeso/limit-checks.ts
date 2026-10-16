import type { LimitView } from "~/api/generated/types";
import type { LimitDraft } from "./limit-editor";

const isRate = (k: string) => k === "request_rate" || k === "token_rate";

export function parentConflict(draft: LimitDraft, parents: LimitView[]): LimitView | null {
  const amount = Number(draft.amount);
  if (!Number.isFinite(amount) || amount <= 0) return null;
  const period = isRate(draft.kind) ? "minute" : draft.period;
  const over = parents.filter((p) => p.kind === draft.kind && p.period === period && (p.model == null || p.model === draft.model) && Number(p.amount) < amount);
  return over.sort((a, b) => Number(a.amount) - Number(b.amount))[0] ?? null;
}

export function parentFilters(target: { level: "project" | "member" | "key" }, owner?: { kind: "user" | "service_account"; id: string }) {
  if (target.level === "project") return [];
  const project = { level: "project" as const };
  if (target.level === "member" || !owner) return [project];
  return [project, owner.kind === "user" ? { user: owner.id, level: "member" as const } : { service_account: owner.id, level: "member" as const }];
}
