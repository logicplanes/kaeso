import { SettingsGroup, SettingsRow, Section, Status, Switch } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { SettingsView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { fullDayLabel } from "~/format/period";
import { t } from "~/i18n";

export function ExchangeRatesSettings({ settings }: { settings: SettingsView }) {
  const rates = settings.exchange_rates;
  const save = useApiMutation({
    mutationFn: async (exchange_rates: boolean) => data(await api.PATCH("/platform/v1/instance/settings", { body: { exchange_rates } })),
    invalidates: [["instance"], ["exchange-rates"]],
    onSuccess: (s) =>
      toast.success(
        s.exchange_rates.fetch
          ? t("instance.ratesOn", "Kaeso fetches the exchange rates daily")
          : t("instance.ratesOff", "Kaeso no longer fetches exchange rates"),
      ),
  });
  const fetch = save.isPending && save.variables !== undefined ? save.variables : rates.fetch;
  return (
    <Section
      spacing="settings"
      title={t("instance.exchangeRates", "Exchange rates")}
      description={t("instance.exchangeRatesHint", "The ECB's daily rates turn USD into other currencies for prices and spend.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("instance.fetchRates", "Fetch exchange rates")}
          description={t("instance.fetchRatesHint", "Off keeps the rates Kaeso has; without any, only USD.")}
          control={
            <Switch
              aria-label={t("instance.fetchRates", "Fetch exchange rates")}
              checked={fetch}
              disabled={save.isPending}
              onCheckedChange={(on) => save.mutate(on)}
            />
          }
        />
        <SettingsRow
          title={t("instance.latestRates", "Latest rates")}
          {...(rates.error ? { description: t("instance.ratesFailed", "The last fetch failed: {error}", { error: rates.error }) } : {})}
          control={
            <Status tone={rates.error ? "warning" : rates.day ? "success" : "neutral"}>
              {rates.day ? fullDayLabel(rates.day) : t("instance.noRates", "None yet")}
            </Status>
          }
        />
      </SettingsGroup>
    </Section>
  );
}
