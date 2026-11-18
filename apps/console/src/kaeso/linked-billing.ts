import { readStoredJson, writeStored } from "~/storage";

const KEY = "kaeso.linked-billing";

export interface PendingLink {
  org: string;
  attempt: string;
  credential?: string;
}

const isPending = (v: unknown): v is PendingLink => {
  if (typeof v !== "object" || v === null) return false;
  const o = v as Record<string, unknown>;
  return typeof o.org === "string" && typeof o.attempt === "string" && (o.credential === undefined || typeof o.credential === "string");
};

// The provider sends the browser back to the install's callback, which finishes the link with
// the attempt that began it; the attempt alone proves nothing without the person's session.
export function rememberLink(link: PendingLink): boolean {
  return writeStored(KEY, JSON.stringify(link), "session");
}

export function takeLink(): PendingLink | null {
  const stored = readStoredJson(KEY, "session");
  writeStored(KEY, null, "session");
  return isPending(stored) ? stored : null;
}
