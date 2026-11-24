import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Banner, Section, Select, SettingsGroup, SettingsRow, Status, Switch } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { api, data } from "~/api/client";
import type { NotificationDelivery } from "~/api/generated/types";
import { notificationsQuery } from "~/api/queries/me";
import { useApiMutation } from "~/auth/mutation";
import { LoadFailed } from "~/features/org/load-failed";
import { areaName, deliveryOptions, kindWords } from "~/kaeso/notification-kinds";
import { SettingsLoading, SettingsPage } from "~/shell/settings-page";
import { t } from "~/i18n";

type Change = { kind: string; delivery: NotificationDelivery };

export function NotificationsPage() {
  const queryClient = useQueryClient();
  const current = useQuery(notificationsQuery);
  const save = useApiMutation({
    mutationFn: async ({ kind, delivery }: Change) => data(await api.PUT("/platform/v1/me/notifications", { body: { kinds: { [kind]: delivery } } })),
    onSuccess: (saved) => {
      queryClient.setQueryData(notificationsQuery.queryKey, saved);
      toast.success(t("account.notificationsSaved", "Notifications saved"));
    },
  });
  const settings = current.data;
  const sends = settings?.sends_email ?? false;
  const pending = save.isPending ? save.variables : undefined;
  return (
    <SettingsPage
      title={t("account.notifications", "Notifications")}
      context={t("account.title", "Account")}
      description={t("account.notificationsPageHint", "The emails Kaeso sends you, and when they go out.")}
    >
      {current.isError ? (
        <LoadFailed title={t("account.notificationsFailed", "Your notification settings couldn't load")} />
      ) : !settings ? (
        <SettingsLoading rows={4} />
      ) : (
        <>
          {!sends && (
            <Banner tone="neutral" title={t("account.noEmailTitle", "This install doesn't send email")}>
              {t("account.noEmailHint", "It writes emails to its log instead; ask an instance admin to set up SMTP.")}
            </Banner>
          )}
          {settings.areas.map((area) => (
            <Section key={area.area} spacing="settings" title={areaName(area)}>
              <SettingsGroup>
                {area.kinds.map((kind) => {
                  const words = kindWords(kind);
                  const value = pending?.kind === kind.kind ? pending.delivery : kind.delivery;
                  return (
                    <SettingsRow
                      key={kind.kind}
                      title={words.name}
                      description={words.description}
                      control={
                        kind.always ? (
                          <Status tone="neutral">{t("account.alwaysSent", "Always sent")}</Status>
                        ) : kind.scheduled ? (
                          <Switch
                            aria-label={words.name}
                            checked={value !== "off"}
                            disabled={!sends || save.isPending}
                            onCheckedChange={(on) => save.mutate({ kind: kind.kind, delivery: on ? "now" : "off" })}
                          />
                        ) : (
                          <Select
                            aria-label={words.name}
                            size="sm"
                            value={value}
                            disabled={!sends || save.isPending}
                            onValueChange={(v) => save.mutate({ kind: kind.kind, delivery: v as NotificationDelivery })}
                            options={deliveryOptions(kind.deliveries)}
                          />
                        )
                      }
                    />
                  );
                })}
              </SettingsGroup>
            </Section>
          ))}
        </>
      )}
    </SettingsPage>
  );
}
