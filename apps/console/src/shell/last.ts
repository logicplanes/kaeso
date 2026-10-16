import { readStoredJson, writeStored } from "~/storage";

export const LAST_PLACE_KEY = "kaeso:last";

export interface LastPlace {
  org: string;
  project?: string;
}

export function readLast(): LastPlace | null {
  const value = readStoredJson(LAST_PLACE_KEY) as Partial<LastPlace> | null;
  if (typeof value?.org !== "string" || !value.org) return null;
  return typeof value.project === "string" && value.project ? { org: value.org, project: value.project } : { org: value.org };
}

export function writeLast(place: LastPlace): void {
  writeStored(LAST_PLACE_KEY, JSON.stringify(place));
}
