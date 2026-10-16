import { useQuery, useQueryClient } from "@tanstack/react-query";
import { SettingsGroup, SettingsRow, SettingsSection, Switch } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { api, data } from "~/api/client";
import type { Notifications } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { LoadFailed } from "~/features/org/load-failed";
import { SettingsLoading, SettingsPage } from "~/shell/settings-page";
import { t } from "~/i18n";

const notificationsQuery = {
  queryKey: ["me", "notifications"],
  queryFn: async (): Promise<Notifications> => data(await api.GET("/platform/v1/me/notifications")),
};

export function NotificationsPage() {
  const queryClient = useQueryClient();
  const current = useQuery(notificationsQuery);
  const save = useApiMutation({
    mutationFn: async (next: Notifications) => data(await api.PUT("/platform/v1/me/notifications", { body: next })),
    onSuccess: (saved) => {
      queryClient.setQueryData(notificationsQuery.queryKey, saved);
      toast.success(t("account.notificationsSaved", "Notifications saved"));
    },
  });
  const n = save.isPending ? save.variables : current.data;
  return (
    <SettingsPage title={t("account.notifications", "Notifications")} context={t("account.title", "Account")}>
      {current.isError ? (
        <LoadFailed title={t("account.notificationsFailed", "Your notification settings couldn't load")} />
      ) : !n ? (
        <SettingsLoading rows={2} />
      ) : (
        <SettingsSection
          title={t("common.email", "Email")}
          description={t("account.emailNotificationsHint", "To your account's address; security emails are always sent.")}
        >
          <SettingsGroup>
            <SettingsRow
              title={t("account.budgetAlerts", "Budget alerts")}
              description={t("account.budgetAlertsHint", "When a budget you're responsible for crosses a threshold.")}
              control={
                <Switch
                  aria-label={t("account.budgetAlerts", "Budget alerts")}
                  checked={n.budget_alerts}
                  disabled={save.isPending}
                  onCheckedChange={(v) => save.mutate({ ...n, budget_alerts: v })}
                />
              }
            />
            <SettingsRow
              title={t("account.expiringKeys", "Expiring keys")}
              description={t("account.expiringKeysHint", "Before a key you own expires.")}
              control={
                <Switch
                  aria-label={t("account.expiringKeys", "Expiring keys")}
                  checked={n.key_expiry}
                  disabled={save.isPending}
                  onCheckedChange={(v) => save.mutate({ ...n, key_expiry: v })}
                />
              }
            />
          </SettingsGroup>
        </SettingsSection>
      )}
    </SettingsPage>
  );
}
