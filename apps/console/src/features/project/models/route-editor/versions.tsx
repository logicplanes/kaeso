import { useMemo } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { Badge, Banner, Button, Collapsible, Empty, List, Text, icons } from "@logicplanes/kaeso-ui";
import { DiffView } from "@logicplanes/kaeso-ui/code-block";
import { FlowWorkspace } from "@logicplanes/kaeso-ui/flow-canvas";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import type { AuditEventView, CatalogueModelView, RouteStep } from "~/api/generated/types";
import { auditLogQuery } from "~/api/queries/audit";
import { usePartyName } from "~/features/project/keys/party";
import { t } from "~/i18n";
import { routeSummary } from "../route-summary";
import { modelRouteOf } from "./route-json";

export type Version = { id: string; at: string; created: boolean; actor: string; name: string; route: RouteStep };

function versionOf(e: AuditEventView, actor: string): Version | undefined {
  const after = e.after as { name?: unknown; route?: unknown; catalogue_model_id?: unknown } | null | undefined;
  if (!after || typeof after.name !== "string") return undefined;
  const route = (after.route as RouteStep | undefined) ?? (typeof after.catalogue_model_id === "string" ? modelRouteOf(after.catalogue_model_id) : undefined);
  if (!route) return undefined;
  return { id: e.id, at: e.at, created: e.action === "alias.create", actor, name: after.name, route };
}

export function useVersions(org: string, project: string, alias: string | undefined, enabled: boolean) {
  const log = useInfiniteQuery({ ...auditLogQuery({ org, project }, { target: alias ?? "", action: "alias." }), enabled: enabled && !!alias });
  const partyName = usePartyName(org, project);
  const versions = useMemo(
    () =>
      (log.data?.pages ?? [])
        .flatMap((p) => p.data)
        .flatMap((e) => {
          const v = versionOf(e, partyName({ kind: e.actor.kind === "service_account" ? "service_account" : "user", id: e.actor.id }));
          return v ? [v] : [];
        }),
    [log.data, partyName],
  );
  return { versions, loading: log.isPending && enabled };
}

export function VersionsPanel({
  versions,
  loading,
  selected,
  current,
  readOnly,
  models,
  onSelect,
  onRestore,
}: {
  versions: Version[];
  loading: boolean;
  selected: string | null;
  current: RouteStep;
  readOnly: boolean;
  models: ReadonlyMap<string, CatalogueModelView>;
  onSelect: (id: string | null) => void;
  onRestore: (v: Version) => void;
}) {
  const chosen = versions.find((v) => v.id === selected);
  const same = chosen ? JSON.stringify(chosen.route) === JSON.stringify(current) : false;
  return (
    <FlowWorkspace.Panel
      title={t("models.route.versions", "Versions")}
      description={t("models.route.versionsHint", "Every save of this alias; choose one to see it on the route.")}
    >
      {chosen && (
        <Banner
          title={t("models.route.showingVersion", "Showing the version {who} saved {when}", { who: chosen.actor, when: formatRelative(chosen.at) })}
          actions={
            <span className="flex flex-wrap gap-8">
              {!readOnly && !same && (
                <Button size="sm" variant="secondary" leadingIcon={icons.undo} onClick={() => onRestore(chosen)}>
                  {t("models.route.restore", "Restore this version")}
                </Button>
              )}
              <Button size="sm" variant="ghost" onClick={() => onSelect(null)}>
                {t("models.route.showMine", "Show your route")}
              </Button>
            </span>
          }
        >
          {same
            ? t("models.route.sameAsYours", "It routes like your route does.")
            : t("models.route.notYours", "Your route differs; restore this version to edit it.")}
        </Banner>
      )}
      {versions.length === 0 && !loading ? (
        <Empty
          title={t("models.route.noVersions", "No versions yet")}
          description={t("models.route.noVersionsHint", "Each save is kept here, with who made it and when.")}
        />
      ) : (
        <List
          aria-label={t("models.route.versions", "Versions")}
          loading={loading}
          onOpen={(item) => onSelect(item.id === selected ? null : item.id)}
          items={versions.map((v, i) => ({
            id: v.id,
            textValue: v.actor,
            title: v.created
              ? t("models.route.createdBy", "Created by {who}", { who: v.actor })
              : t("models.route.savedBy", "Saved by {who}", { who: v.actor }),
            subtitle: (
              <Text variant="small" tone="secondary" truncate>
                {routeSummary(v.route, models)}
              </Text>
            ),
            meta: (
              <span className="flex items-center gap-6">
                {i === 0 && <Badge tone="success">{t("models.route.live", "Live")}</Badge>}
                {v.id === selected && <Badge>{t("models.route.shownVersion", "Shown")}</Badge>}
              </span>
            ),
            date: formatRelative(v.at),
          }))}
        />
      )}
      {chosen && !same && (
        <Collapsible title={t("models.route.changesAsJson", "Changes as JSON")}>
          <DiffView
            title={t("models.route.diffTitle", "This version → your route")}
            before={JSON.stringify(chosen.route, null, 2)}
            after={JSON.stringify(current, null, 2)}
            lang="json"
            wrap
          />
        </Collapsible>
      )}
    </FlowWorkspace.Panel>
  );
}
