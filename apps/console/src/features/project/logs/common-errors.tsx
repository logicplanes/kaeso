import { useQuery } from "@tanstack/react-query";
import { Collapsible, List, Text } from "@logicplanes/kaeso-ui";
import type { ErrorGroupView } from "~/api/generated/types";
import { logErrorsQuery, type LogFilters } from "~/api/queries/logs";
import { errorKindLabel } from "~/format/labels";
import { t } from "~/i18n";

const MAX_MESSAGE = 200;

/**
 * The part of a message that names the failure, before its first number or id, so it finds
 * the same failure with other numbers: "Rate limit reached; retry after 12 s" → "Rate limit
 * reached; retry after". Nothing when the message starts with one.
 */
export function messagePrefix(message: string): string | undefined {
  const cut = message.search(/req_|[0-9]/);
  const head = (cut < 0 ? message : message.slice(0, cut)).replace(/[\s([{:;,.-]+$/, "").trim();
  return head ? head.slice(0, MAX_MESSAGE) : undefined;
}

/** The filters a row of the most common errors sets, to list the requests it counts; none it leaves unset. */
export function filtersOf(g: ErrorGroupView): { error: string | undefined; status: number; provider: string | undefined; error_message: string | undefined } {
  return {
    error: g.error_kind || undefined,
    status: g.status,
    provider: g.provider || undefined,
    error_message: messagePrefix(g.example),
  };
}

export function CommonErrors({
  org,
  project,
  filters,
  providerName,
  onPick,
}: {
  org: string;
  project: string;
  filters: LogFilters;
  providerName: (id: string) => string;
  onPick: (g: ErrorGroupView) => void;
}) {
  const errors = useQuery(logErrorsQuery(org, project, filters));
  if (errors.isError)
    return (
      <Text variant="small" tone="secondary">
        {t("logs.commonErrorsFailed", "The most common errors couldn't load.")}
      </Text>
    );
  const groups = errors.data ?? [];
  if (errors.isSuccess && groups.length === 0) return null;
  return (
    <Collapsible defaultOpen title={t("logs.commonErrors", "Most common errors")} hint={t("logs.commonErrorsHint", "Choose one to list its requests")}>
      <List
        aria-label={t("logs.commonErrors", "Most common errors")}
        loading={errors.isPending}
        items={groups.map((g, i) => ({
          id: String(i),
          textValue: g.example || errorKindLabel(g.error_kind),
          title: <Text truncate>{g.example || errorKindLabel(g.error_kind)}</Text>,
          subtitle: [errorKindLabel(g.error_kind), String(g.status), ...(g.provider ? [providerName(g.provider)] : [])].join(" · "),
          meta: t("logs.commonErrorsCount", "{count, plural, one {# request} other {# requests}}", { count: g.count }),
        }))}
        onOpen={(item) => onPick(groups[Number(item.id)] as ErrorGroupView)}
      />
    </Collapsible>
  );
}
