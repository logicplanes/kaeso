import { playgroundDraftKey } from "~/auth/user-data";
import { readStored, writeStored } from "~/storage";

/** The prompt someone typed in a project's Playground and hasn't sent, kept in this browser. */
export function readDraft(project: string): string {
  return readStored(playgroundDraftKey(project)) ?? "";
}

export function writeDraft(project: string, text: string) {
  writeStored(playgroundDraftKey(project), text || null);
}
