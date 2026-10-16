import { parseDay } from "~/format/period";

export interface AuditFilters {
  actor?: string;
  action?: string;
  target?: string;
  from?: string;
  to?: string;
}

export type AuditSearch = AuditFilters & { event?: string };

export function validateAuditSearch(s: Record<string, unknown>): AuditSearch {
  const str = (v: unknown) => (typeof v === "string" && v.trim() ? v.trim() : undefined);
  const from = parseDay(s.from);
  const to = parseDay(s.to);
  const out: AuditSearch = {};
  const actor = str(s.actor);
  const action = str(s.action);
  const target = str(s.target);
  const event = str(s.event);
  if (actor) out.actor = actor;
  if (action) out.action = action;
  if (target) out.target = target;
  if (from && to && from <= to) Object.assign(out, { from, to });
  if (event) out.event = event;
  return out;
}
