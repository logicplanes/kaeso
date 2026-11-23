import { useCallback } from "react";
import { useQuery } from "@tanstack/react-query";
import type { ExternalSourceView } from "~/api/generated/types";
import { sourcesQuery } from "~/api/queries/sources";
import { useProjectRights } from "~/auth/rights";
import { t } from "~/i18n";

export interface SourceNames {
  /** The source with this id, archived ones included; none for one the person can't see. */
  source: (id: string | null | undefined) => ExternalSourceView | undefined;
  /** Its name, or "A source" for one the person can't see. */
  name: (id: string) => string;
  sources: readonly ExternalSourceView[];
  /** The sources are still loading, so a name may be a placeholder for a moment. */
  loading: boolean;
}

/** The request log's filter for a key that usage grouped by `key` returns: the Playground, a source's requests, or a key. */
export function keyLogFilter(key: string): Record<string, string> {
  if (key === "playground") return { source: "playground" };
  if (key.startsWith("source:")) return { source_id: key.slice(7) };
  return { key };
}

/** The project's sources by id, for the logs and analytics that name them; nothing without the right to see them. */
export function useSourceNames(org: string, project: string): SourceNames {
  const rights = useProjectRights(org, project);
  const query = useQuery({ ...sourcesQuery(org, project), enabled: !!project && rights.has("sources.view") });
  const sources = query.data;
  const source = useCallback((id: string | null | undefined) => (id ? sources?.find((s) => s.id === id) : undefined), [sources]);
  const name = useCallback((id: string) => sources?.find((s) => s.id === id)?.name ?? t("project.analytics.aSource", "A source"), [sources]);
  return { source, name, sources: sources ?? [], loading: query.isLoading };
}
