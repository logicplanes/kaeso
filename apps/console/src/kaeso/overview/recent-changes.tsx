import { useInfiniteQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { RankList, type RankItem } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import type { AuditEventView } from "~/api/generated/types";
import { auditLogQuery, type AuditScope } from "~/api/queries/audit";
import { auditActionLabel, snapshotName } from "~/format/audit";
import { t } from "~/i18n";

export function RecentChanges({
  scope,
  href,
  actorName,
  rows = 5,
}: {
  scope: AuditScope;
  href: string;
  actorName: (e: AuditEventView) => string;
  rows?: number;
}) {
  const navigate = useNavigate();
  const log = useInfiniteQuery(auditLogQuery(scope, {}));
  const events = (log.data?.pages[0]?.data ?? []).slice(0, rows);
  const items: RankItem[] = events.map((e) => {
    const target = snapshotName(e);
    return {
      id: e.id,
      label: (
        <span className="min-w-0 truncate">
          {target
            ? t("kaeso.home.changeOn", "{actor} · {action} · {target}", { actor: actorName(e), action: auditActionLabel(e.action), target })
            : t("kaeso.home.change", "{actor} · {action}", { actor: actorName(e), action: auditActionLabel(e.action) })}
        </span>
      ),
      secondary: formatRelative(e.at),
    };
  });
  return (
    <RankList
      aria-label={t("kaeso.home.recentChanges", "Recent changes")}
      items={items}
      loading={log.isPending}
      rows={rows}
      empty={t("kaeso.home.noChanges", "No changes yet")}
      onOpen={(id) => void navigate({ href: `${href}?event=${encodeURIComponent(id)}` })}
    />
  );
}
