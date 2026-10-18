import { useMemo } from "react";
import { Button, Combobox, SettingsGroup, SettingsRow, SettingsSection, Text } from "@logicplanes/kaeso-ui";
import { setFormatTimeZone } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { SettingsView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { browserTimeZone, clockIn, offsetOf, timeZoneNames } from "~/format/time-zones";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

export function TimeZoneSettings({ settings }: { settings: SettingsView }) {
  const zone = settings.timezone;
  const browser = browserTimeZone();
  const options = useMemo(() => {
    const now = new Date();
    return timeZoneNames(zone).map((name) => ({ value: name, label: name, description: offsetOf(name, now) }));
  }, [zone]);
  const save = useApiMutation({
    mutationFn: async (timezone: string) => data(await api.PATCH("/platform/v1/instance/settings", { body: { timezone } })),
    invalidates: [["instance"], ["session"]],
    onSuccess: (s) => {
      setFormatTimeZone(s.timezone);
      toast.success(t("instance.timeZoneSaved", "Times now show in {zone}", { zone: s.timezone }));
    },
  });
  const choose = (next: string | null) => next && next !== zone && save.mutate(next);
  return (
    <SettingsSection
      title={t("instance.timeZone", "Time zone")}
      description={t("instance.timeZoneHint", "Every time in the console and the automatic update window follow it.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("instance.timeZone", "Time zone")}
          description={t("instance.timeZoneRowHint", "For everyone on this install, whatever their own zone.")}
          control={
            <div className="w-240">
              <Combobox
                aria-label={t("instance.timeZone", "Time zone")}
                size="sm"
                value={zone}
                options={options}
                disabled={save.isPending}
                emptyText={t("instance.noTimeZone", "No time zone matches")}
                onValueChange={choose}
              />
            </div>
          }
        />
        <SettingsRow
          title={t("instance.timeNow", "Time there now")}
          control={
            <Text tabular tone="secondary">
              {t("instance.timeNowValue", "{clock} ({offset})", { clock: clockIn(zone), offset: offsetOf(zone) })}
            </Text>
          }
        />
        {browser !== zone && (
          <SettingsRow
            title={t("instance.browserZone", "Your browser's time zone")}
            description={t("instance.browserZoneHint", "{zone}, {offset}", { zone: browser, offset: offsetOf(browser) })}
            control={
              <Button size="sm" loading={save.isPending} onClick={() => choose(browser)}>
                {t("instance.useBrowserZone", "Use this zone")}
              </Button>
            }
          />
        )}
      </SettingsGroup>
    </SettingsSection>
  );
}
