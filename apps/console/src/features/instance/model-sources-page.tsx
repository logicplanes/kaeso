import type { ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { Badge, Button, DataTable, StatusDot, Text, Tooltip, col, icons } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { ModelsDevStatus } from "~/api/generated/types";
import { modelCatalogues } from "~/api/queries/invalidation";
import { modelsDevQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { usePhone } from "~/shell/use-phone";
import { InstancePage } from "./instance-page";
import { t } from "~/i18n";

const every = (seconds: number) =>
  seconds % 3600 === 0
    ? t("instance.everyHours", "Every {count} h", { count: seconds / 3600 })
    : seconds % 60 === 0
      ? t("instance.everyMinutes", "Every {count} min", { count: seconds / 60 })
      : t("instance.everySeconds", "Every {count} s", { count: seconds });

type Source = { id: string; name: string; inUse: boolean; status: ModelsDevStatus };

function Stacked({ children, detail }: { children: ReactNode; detail: ReactNode }) {
  return (
    <span className="flex min-w-0 flex-col">
      {children}
      <Text variant="small" tone="secondary" truncate>
        {detail}
      </Text>
    </span>
  );
}

const DATA_WIDTH = 176;
const PHONE_DATA_WIDTH = 152;

export function InstanceModelSourcesPage() {
  const phone = usePhone();
  const status = useQuery(modelsDevQuery);
  const sync = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/instance/models-dev/sync")),
    invalidates: [modelsDevQuery.queryKey, modelCatalogues],
    onSuccess: (s) =>
      s.last_error
        ? toast.warning(t("instance.syncFailed", "Couldn't fetch models.dev; the current data stays in use"))
        : toast.success(t("instance.synced", "Synced: {count, plural, one {# model} other {# models}}", { count: s.model_count })),
  });
  const rows: Source[] = status.data ? [{ id: "models.dev", name: "models.dev", inUse: true, status: status.data }] : [];

  const columns = [
    col.custom(
      { id: "source", header: t("instance.source", "Source"), size: 208, priority: 1, grow: true },
      (r: Source) => r.name,
      (name: string, r) => (
        <Stacked detail={r.status.url}>
          <span className="flex min-w-0 items-center gap-8">
            <Text weight="medium" truncate>
              {name}
            </Text>
            {r.inUse && <Badge tone="success">{t("instance.inUse", "In use")}</Badge>}
          </span>
        </Stacked>
      ),
    ),
    col.custom(
      { id: "data", header: t("instance.dataInUse", "Data in use"), size: phone ? PHONE_DATA_WIDTH : DATA_WIDTH, priority: 1 },
      (r: Source) => r.status,
      (s: ModelsDevStatus) => (
        <Stacked
          detail={
            s.source === "embedded"
              ? t("instance.snapshot", "Snapshot, {when}", { when: formatRelative(s.fetched_at) })
              : t("instance.fetched", "Fetched {when}", { when: formatRelative(s.fetched_at) })
          }
        >
          <Text truncate>{t("instance.modelCount", "{count, plural, one {# model} other {# models}}", { count: s.model_count })}</Text>
        </Stacked>
      ),
    ),
    col.custom(
      { id: "attempt", header: t("instance.lastSync", "Last sync"), size: 136, priority: 2 },
      (r: Source) => r.status,
      (s: ModelsDevStatus) =>
        s.last_attempt_at ? (
          <Stacked detail={formatRelative(s.last_attempt_at)}>
            {s.last_error ? (
              <Tooltip content={s.last_error}>
                <span className="inline-flex">
                  <StatusDot tone="danger">{t("instance.failed", "Failed")}</StatusDot>
                </span>
              </Tooltip>
            ) : (
              <StatusDot tone="success">{t("instance.succeeded", "Succeeded")}</StatusDot>
            )}
          </Stacked>
        ) : (
          <Text tone="secondary">{t("instance.noneYet", "None yet")}</Text>
        ),
    ),
    col.custom(
      { id: "auto", header: t("instance.autoSync", "Automatic sync"), size: 144, priority: 2 },
      (r: Source) => r.status,
      (s: ModelsDevStatus) => (
        <Stacked detail={t("instance.gatewaySetting", "Gateway setting")}>
          <StatusDot tone={s.sync_enabled ? "success" : "neutral"}>{s.sync_enabled ? every(s.interval_seconds) : t("common.off", "Off")}</StatusDot>
        </Stacked>
      ),
    ),
  ].map((c) => ({ ...c, enableSorting: false }));

  return (
    <InstancePage
      title={t("instance.modelSources", "Model sources")}
      description={t("instance.modelSourcesHint", "Where every project's model names, limits and prices come from.")}
      actions={
        <Button size="sm" leadingIcon={icons.refresh} loading={sync.isPending} disabled={!status.data} onClick={() => sync.mutate()}>
          {t("instance.syncNow", "Sync now")}
        </Button>
      }
    >
      <DataTable
        className="mt-24"
        aria-label={t("instance.modelSources", "Model sources")}
        data={rows}
        columns={columns}
        getRowId={(r) => r.id}
        density="comfortable"
        loading={status.isPending}
        error={status.isError && rows.length === 0 ? t("instance.modelSourcesFailed", "Model sources couldn't load") : undefined}
        onRetry={() => void status.refetch()}
      />
    </InstancePage>
  );
}
