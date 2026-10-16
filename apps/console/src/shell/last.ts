import { readStoredJson, writeStored } from "~/storage";

const KEY = "kaeso:last";

export interface LastPlace {
  org: string;
  project?: string;
}

export function readLast(): LastPlace | null {
  const value = readStoredJson(KEY) as Partial<LastPlace> | null;
  if (typeof value?.org !== "string" || !value.org) return null;
  return typeof value.project === "string" && value.project ? { org: value.org, project: value.project } : { org: value.org };
}

export function writeLast(place: LastPlace): void {
  writeStored(KEY, JSON.stringify(place));
}
