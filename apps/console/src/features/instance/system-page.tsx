import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button, Code, SettingsGroup, SettingsRow, SettingsSection, StatusDot, Text, Tooltip, icons } from "@logicplanes/kaeso-ui";
import { formatBytes, formatNumber, formatRelative } from "@logicplanes/kaeso-ui/format";
import { systemQuery } from "~/api/queries/instance";
import { SettingsLoading } from "~/shell/settings-page";
import { InstancePage } from "./instance-page";
import { t } from "~/i18n";

const statuses = (): Record<string, { tone: "success" | "warning" | "danger"; label: string }> => ({
  ready: { tone: "success", label: t("instance.ready", "Ready") },
  degraded: { tone: "warning", label: t("instance.degraded", "Degraded") },
  not_ready: { tone: "danger", label: t("instance.notReady", "Not ready") },
});

const SERVICE = { postgres: "PostgreSQL", clickhouse: "ClickHouse", valkey: "Valkey" };

export function InstanceSystemPage() {
  const system = useQuery({ ...systemQuery, refetchInterval: 10_000 });
  const [checking, setChecking] = useState(false);
  const s = system.data;
  const status = statuses()[s?.status ?? ""] ?? { tone: "warning" as const, label: s?.status ?? "" };
  const size = (bytes: number | null | undefined) => (bytes == null ? t("common.unknown", "Unknown") : formatBytes(bytes));
  const service = (state: string) => (
    <StatusDot tone={state === "ok" ? "success" : "danger"}>{state === "ok" ? t("instance.up", "Up") : t("instance.down", "Down")}</StatusDot>
  );
  const checkNow = () => {
    setChecking(true);
    void system.refetch().finally(() => setChecking(false));
  };
  return (
    <InstancePage
      title={t("instance.system", "System")}
      actions={
        <Tooltip
          content={
            s
              ? t("instance.lastChecked", "Last checked {when} · every 10 seconds", { when: formatRelative(system.dataUpdatedAt) })
              : t("instance.checkedEvery", "Checked every 10 seconds")
          }
        >
          <Button size="sm" leadingIcon={icons.refresh} loading={checking} onClick={checkNow}>
            {t("instance.checkNow", "Check now")}
          </Button>
        </Tooltip>
      }
    >
      {!s ? (
        <SettingsLoading rows={4} />
      ) : (
        <>
          <SettingsSection title={t("instance.gateway", "Gateway")}>
            <SettingsGroup>
              <SettingsRow title={t("common.status", "Status")} control={<StatusDot tone={status.tone}>{status.label}</StatusDot>} />
              <SettingsRow title={t("instance.version", "Version")} control={<Code>{s.version}</Code>} />
            </SettingsGroup>
          </SettingsSection>
          <SettingsSection title={t("instance.services", "Services")}>
            <SettingsGroup>
              <SettingsRow
                title={SERVICE.postgres}
                description={t("instance.postgresHint", "Users, organizations, keys and settings.")}
                control={service(s.postgres)}
              />
              <SettingsRow title={SERVICE.clickhouse} description={t("instance.clickhouseHint", "Request logs and usage.")} control={service(s.clickhouse)} />
              <SettingsRow title={SERVICE.valkey} description={t("instance.valkeyHint", "Limits and caches.")} control={service(s.valkey)} />
            </SettingsGroup>
          </SettingsSection>
          <SettingsSection title={t("instance.logBuffer", "Request log buffer")}>
            <SettingsGroup>
              <SettingsRow
                title={t("instance.waiting", "Waiting")}
                description={t("instance.waitingHint", "Entries this instance holds until ClickHouse takes them.")}
                control={<Text tabular>{formatNumber(s.log_buffer.rows)}</Text>}
              />
              <SettingsRow
                title={t("instance.dropped", "Dropped since start")}
                description={
                  s.log_buffer.dropped > 0
                    ? t("instance.droppedHint", "The buffer was full: these requests have no log entry.")
                    : t("instance.droppedNone", "None: every request was logged.")
                }
                control={
                  <Text tabular tone={s.log_buffer.dropped > 0 ? "danger" : "default"}>
                    {formatNumber(s.log_buffer.dropped)}
                  </Text>
                }
              />
            </SettingsGroup>
          </SettingsSection>
          <SettingsSection
            title={t("instance.storage", "Storage")}
            description={t("instance.storageHint", "The whole install; each organization shows its own on its Logging page.")}
          >
            <SettingsGroup>
              <SettingsRow title={SERVICE.postgres} control={<Text tabular>{size(s.storage.postgres_bytes)}</Text>} />
              <SettingsRow title={SERVICE.clickhouse} control={<Text tabular>{size(s.storage.clickhouse_bytes_on_disk)}</Text>} />
              <SettingsRow
                title={t("instance.logEntries", "Request log entries")}
                control={<Text tabular>{s.storage.request_log_rows == null ? t("common.unknown", "Unknown") : formatNumber(s.storage.request_log_rows)}</Text>}
              />
            </SettingsGroup>
          </SettingsSection>
        </>
      )}
    </InstancePage>
  );
}
