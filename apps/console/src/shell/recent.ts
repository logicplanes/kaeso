import { useEffect } from "react";
import type { IconData } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";
import { readStoredJson, writeStored } from "~/storage";

export interface RecentEntity {
  kind: "key" | "model" | "member" | "service-account" | "request";
  id: string;
  label: string;
  href: string;
}

const STORE = "kaeso:recent";
const MAX = 8;

export function readRecent(): RecentEntity[] {
  const v = readStoredJson(STORE);
  return Array.isArray(v) ? (v as RecentEntity[]).filter((e) => e && typeof e.href === "string" && typeof e.label === "string").slice(0, MAX) : [];
}

export function rememberRecent(entity: RecentEntity): void {
  const next = [entity, ...readRecent().filter((e) => !(e.kind === entity.kind && e.id === entity.id))].slice(0, MAX);
  writeStored(STORE, JSON.stringify(next));
}

export function recentHint(kind: RecentEntity["kind"]): string {
  switch (kind) {
    case "key":
      return t("shell.kind.key", "Key");
    case "model":
      return t("common.model", "Model");
    case "member":
      return t("shell.kind.member", "Member");
    case "service-account":
      return t("shell.kind.serviceAccount", "Service account");
    case "request":
      return t("shell.kind.request", "Request");
  }
}

export type RecentIcons = Record<RecentEntity["kind"], IconData>;

export function useRememberRecent(entity: RecentEntity | null): void {
  const kind = entity?.kind;
  const id = entity?.id;
  const label = entity?.label;
  const href = entity?.href;
  useEffect(() => {
    if (kind && id && label && href) rememberRecent({ kind, id, label, href });
  }, [kind, id, label, href]);
}
