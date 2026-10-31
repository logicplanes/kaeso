import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button, Empty, NumberInput, SettingsGroup, SettingsRow, SettingsSection } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { SettingsView } from "~/api/generated/types";
import { instanceSettingsQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { useRequestLogs } from "~/auth/session";
import { toast } from "~/shell/toast";
import { SettingsLoading } from "~/shell/settings-page";
import { LoadFailed } from "~/features/org/load-failed";
import { InstancePage } from "./instance-page";
import { t } from "~/i18n";

export function InstanceLoggingPage() {
  const requestLogs = useRequestLogs();
  const settings = useQuery({ ...instanceSettingsQuery, enabled: requestLogs });
  const s = settings.data;
  return (
    <InstancePage
      title={t("shell.settings.logging", "Logging")}
      description={t("instance.loggingHint", "How long request logs are kept, for new organizations and at most.")}
    >
      {!requestLogs ? (
        <Empty
          size="lg"
          title={t("instance.logsOff", "Request logs are off on this install")}
          description={t("instance.logsOffHint", "Set KAESO_CLICKHOUSE_URL and restart the gateway to keep request logs.")}
        />
      ) : settings.isError ? (
        <LoadFailed title={t("instance.settingsFailed", "The instance settings couldn't load")} />
      ) : !s ? (
        <SettingsLoading rows={4} />
      ) : (
        <Retention
          key={`${s.metadata_retention_days}-${s.content_retention_days}-${s.max_metadata_retention_days}-${s.max_content_retention_days}`}
          settings={s}
        />
      )}
    </InstancePage>
  );
}

type RetentionField = "metadata_retention_days" | "content_retention_days" | "max_metadata_retention_days" | "max_content_retention_days";

function Retention({ settings }: { settings: SettingsView }) {
  const [draft, setDraft] = useState<Record<RetentionField, string>>({
    metadata_retention_days: String(settings.metadata_retention_days),
    content_retention_days: String(settings.content_retention_days),
    max_metadata_retention_days: String(settings.max_metadata_retention_days),
    max_content_retention_days: String(settings.max_content_retention_days),
  });
  const [saving, setSaving] = useState<RetentionField | null>(null);
  const save = useApiMutation({
    mutationFn: async (body: Partial<Record<RetentionField, number>>) => data(await api.PATCH("/platform/v1/instance/settings", { body })),
    invalidates: [["instance", "settings"]],
    onSuccess: () => toast.success(t("instance.retentionSaved", "Retention saved")),
  });
  const next = (field: RetentionField) => ({ ...settings, [field]: Number(draft[field]) });
  const reason = (field: RetentionField): string | undefined => {
    const v = Number(draft[field]);
    const s = next(field);
    if (!Number.isInteger(v) || v < 1) return t("instance.wholeDays", "Enter whole days, at least 1");
    if (v === settings[field]) return t("instance.noChange", "No change to save");
    if (s.metadata_retention_days > s.max_metadata_retention_days) return t("instance.metadataAboveMax", "The default metadata retention is above its maximum");
    if (s.content_retention_days > s.max_content_retention_days) return t("instance.contentAboveMax", "The default content retention is above its maximum");
    if (s.content_retention_days > s.metadata_retention_days || s.max_content_retention_days > s.max_metadata_retention_days)
      return t("instance.contentLongerThanMetadata", "Content can't be kept longer than metadata");
    return undefined;
  };
  const control = (field: RetentionField, label: string, saveLabel: string) => {
    const why = reason(field);
    return (
      <div className="flex min-w-0 items-center gap-8">
        <div className="w-160 min-w-0">
          <NumberInput
            aria-label={label}
            size="sm"
            integer
            value={draft[field]}
            onValueChange={(v) => setDraft((d) => ({ ...d, [field]: v }))}
            min="1"
            step="1"
            unit={t("instance.daysUnit", "days")}
          />
        </div>
        <Button
          size="sm"
          aria-label={saveLabel}
          disabled={!!why}
          {...(why ? { disabledReason: why } : {})}
          loading={save.isPending && saving === field}
          onClick={() => {
            setSaving(field);
            save.mutate({ [field]: Number(draft[field]) });
          }}
        >
          {t("common.save", "Save")}
        </Button>
      </div>
    );
  };
  return (
    <SettingsSection
      title={t("instance.retention", "Request log retention")}
      description={t("instance.retentionHint", "Defaults for new organizations, and the most any organization may choose.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("instance.metadataDefault", "Metadata, by default")}
          description={t("instance.metadataHint", "Timing, tokens, cost and errors.")}
          control={control(
            "metadata_retention_days",
            t("instance.metadataDefaultLabel", "Default metadata retention"),
            t("instance.saveMetadataDefault", "Save default metadata retention"),
          )}
        />
        <SettingsRow
          title={t("instance.contentDefault", "Content, by default")}
          description={t("instance.contentHint", "Request and reply bodies.")}
          control={control(
            "content_retention_days",
            t("instance.contentDefaultLabel", "Default content retention"),
            t("instance.saveContentDefault", "Save default content retention"),
          )}
        />
        <SettingsRow
          title={t("instance.metadataMax", "Metadata, at most")}
          description={t("instance.metadataMaxHint", "No organization keeps metadata longer.")}
          control={control(
            "max_metadata_retention_days",
            t("instance.metadataMaxLabel", "Maximum metadata retention"),
            t("instance.saveMetadataMax", "Save maximum metadata retention"),
          )}
        />
        <SettingsRow
          title={t("instance.contentMax", "Content, at most")}
          description={t("instance.contentMaxHint", "No organization keeps bodies longer.")}
          control={control(
            "max_content_retention_days",
            t("instance.contentMaxLabel", "Maximum content retention"),
            t("instance.saveContentMax", "Save maximum content retention"),
          )}
        />
      </SettingsGroup>
    </SettingsSection>
  );
}
