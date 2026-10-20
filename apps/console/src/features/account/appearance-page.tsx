import { useQueryClient } from "@tanstack/react-query";
import { Select, SettingsGroup, SettingsRow, SettingsSection, icons, useTheme } from "@logicplanes/kaeso-ui";
import { UI_LOCALE_NAMES, UI_LOCALES } from "@logicplanes/kaeso-ui/i18n";
import { api, data } from "~/api/client";
import { useApiMutation } from "~/auth/mutation";
import { sessionQuery, useSession } from "~/auth/session";
import { resolve, t, type LocaleChoice } from "~/i18n";
import { useChangeLocale } from "~/i18n/provider";
import { SettingsPage } from "~/shell/settings-page";

type Theme = "system" | "light" | "dark";

export function AppearancePage() {
  const { theme, setTheme } = useTheme();
  const session = useSession();
  const changeLocale = useChangeLocale();
  const queryClient = useQueryClient();
  const choose = useApiMutation({
    mutationFn: async (locale: LocaleChoice) => data(await api.PATCH("/platform/v1/me", { body: { locale } })).locale,
    invalidates: [["me"]],
    onSuccess: (locale) => {
      queryClient.setQueryData(sessionQuery.queryKey, (s) => (s ? { ...s, locale } : s));
      return changeLocale(locale);
    },
  });
  const choice = choose.isPending && choose.variables ? choose.variables : session.locale;
  return (
    <SettingsPage
      title={t("account.appearance", "Appearance")}
      context={t("account.title", "Account")}
      description={t("account.appearanceHint", "How the console looks in this browser, and the language it speaks to you.")}
    >
      <SettingsSection>
        <SettingsGroup>
          <SettingsRow
            title={t("account.theme", "Theme")}
            description={t("account.themeHint", "System follows your device; saved in this browser.")}
            control={
              <Select
                aria-label={t("account.theme", "Theme")}
                size="sm"
                value={theme}
                onValueChange={(v) => setTheme(v as Theme)}
                options={[
                  { value: "system", label: t("account.themeSystem", "System"), icon: icons.themeSystem },
                  { value: "light", label: t("account.themeLight", "Light"), icon: icons.themeLight },
                  { value: "dark", label: t("account.themeDark", "Dark"), icon: icons.themeDark },
                ]}
              />
            }
          />
          <SettingsRow
            title={t("account.language", "Language")}
            description={t("account.languageHint", "Automatic follows your browser; saved to your account.")}
            control={
              <Select
                aria-label={t("account.language", "Language")}
                size="sm"
                value={choice}
                disabled={choose.isPending}
                onValueChange={(v) => choose.mutate(v as LocaleChoice)}
                options={[
                  { value: "auto", label: t("account.languageAuto", "Automatic ({language})", { language: UI_LOCALE_NAMES[resolve("auto")] }) },
                  ...UI_LOCALES.map((l) => ({ value: l, label: UI_LOCALE_NAMES[l] })),
                ]}
              />
            }
          />
        </SettingsGroup>
      </SettingsSection>
    </SettingsPage>
  );
}
