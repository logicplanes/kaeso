import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button, NumberInput, SettingsGroup, SettingsRow, SettingsSection } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { SettingsView } from "~/api/generated/types";
import { instanceSettingsQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { SettingsLoading } from "~/shell/settings-page";
import { LoadFailed } from "~/features/org/load-failed";
import { InstancePage } from "./instance-page";
import { t } from "~/i18n";

export function InstanceLoggingPage() {
  const settings = useQuery(instanceSettingsQuery);
  const s = settings.data;
  return (
    <InstancePage
      title={t("shell.settings.logging", "Logging")}
      description={t("instance.loggingHint", "How long request logs are kept, for new organizations and at most.")}
    >
      {settings.isError ? (
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

function Retention({ settings }: { settings: SettingsView }) {
  const [metadata, setMetadata] = useState(String(settings.metadata_retention_days));
  const [content, setContent] = useState(String(settings.content_retention_days));
  const [maxMetadata, setMaxMetadata] = useState(String(settings.max_metadata_retention_days));
  const [maxContent, setMaxContent] = useState(String(settings.max_content_retention_days));
  const save = useApiMutation({
    mutationFn: async () =>
      data(
        await api.PATCH("/platform/v1/instance/settings", {
          body: {
            metadata_retention_days: Number(metadata),
            content_retention_days: Number(content),
            max_metadata_retention_days: Number(maxMetadata),
            max_content_retention_days: Number(maxContent),
          },
        }),
      ),
    invalidates: [["instance", "settings"]],
    onSuccess: () => toast.success(t("instance.retentionSaved", "Retention saved")),
  });
  const [m, c, mm, mc] = [metadata, content, maxMetadata, maxContent].map(Number) as [number, number, number, number];
  const whole = (n: number) => Number.isInteger(n) && n >= 1;
  const changed =
    m !== settings.metadata_retention_days ||
    c !== settings.content_retention_days ||
    mm !== settings.max_metadata_retention_days ||
    mc !== settings.max_content_retention_days;
  const reason = ![m, c, mm, mc].every(whole)
    ? t("instance.wholeDays", "Enter whole days, at least 1")
    : m > mm
      ? t("instance.metadataAboveMax", "The default metadata retention is above its maximum")
      : c > mc
        ? t("instance.contentAboveMax", "The default content retention is above its maximum")
        : c > m || mc > mm
          ? t("instance.contentLongerThanMetadata", "Content can't be kept longer than metadata")
          : !changed
            ? t("instance.noChange", "No change to save")
            : undefined;
  const field = (label: string, value: string, set: (v: string) => void) => (
    <div className="w-160">
      <NumberInput aria-label={label} size="sm" value={value} onValueChange={set} min="1" step="1" unit={t("instance.daysUnit", "days")} />
    </div>
  );
  return (
    <SettingsSection
      title={t("instance.retention", "Request log retention")}
      description={t("instance.retentionHint", "Defaults for new organizations, and the most any organization may choose.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("instance.metadataDefault", "Metadata, by default")}
          description={t("instance.metadataHint", "Timing, tokens, cost and errors.")}
          control={field(t("instance.metadataDefaultLabel", "Default metadata retention"), metadata, setMetadata)}
        />
        <SettingsRow
          title={t("instance.contentDefault", "Content, by default")}
          description={t("instance.contentHint", "Request and reply bodies.")}
          control={field(t("instance.contentDefaultLabel", "Default content retention"), content, setContent)}
        />
        <SettingsRow
          title={t("instance.metadataMax", "Metadata, at most")}
          control={field(t("instance.metadataMaxLabel", "Maximum metadata retention"), maxMetadata, setMaxMetadata)}
        />
        <SettingsRow
          title={t("instance.contentMax", "Content, at most")}
          control={field(t("instance.contentMaxLabel", "Maximum content retention"), maxContent, setMaxContent)}
        />
      </SettingsGroup>
      <div className="mt-12 flex justify-end">
        <Button
          size="sm"
          variant="primary"
          disabled={!!reason}
          {...(reason ? { disabledReason: reason } : {})}
          loading={save.isPending}
          onClick={() => save.mutate()}
        >
          {t("instance.saveRetention", "Save retention")}
        </Button>
      </div>
    </SettingsSection>
  );
}
