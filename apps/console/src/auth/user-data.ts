import { LAST_PLACE_KEY } from "~/shell/last";
import { RECENT_KEY } from "~/shell/recent";
import { writeStored } from "~/storage";

export const INVITE_KEY = "kaeso:invite";

const DRAFT_PREFIX = "kaeso:playground-draft:";

export const playgroundDraftKey = (project: string): string => `${DRAFT_PREFIX}${project}`;

function storedDrafts(): string[] {
  try {
    return Object.keys(window.localStorage).filter((key) => key.startsWith(DRAFT_PREFIX));
  } catch {
    return [];
  }
}

export function forgetUserData(): void {
  for (const key of [RECENT_KEY, LAST_PLACE_KEY, ...storedDrafts()]) writeStored(key, null);
  writeStored(INVITE_KEY, null, "session");
}
