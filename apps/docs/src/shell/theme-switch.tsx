import { Segmented, icons, useTheme, type ThemePreference } from "@logicplanes/kaeso-ui";
import { useUiText } from "@logicplanes/kaeso-ui/i18n";

export function ThemeSwitch() {
  const t = useUiText();
  const { theme, setTheme } = useTheme();
  return (
    <Segmented
      aria-label={t("themeMenu.theme", "Theme")}
      size="sm"
      value={theme}
      onValueChange={(value) => setTheme(value as ThemePreference)}
      options={[
        { value: "system", icon: icons.themeSystem, "aria-label": t("themeMenu.system", "System") },
        { value: "light", icon: icons.themeLight, "aria-label": t("themeMenu.light", "Light") },
        { value: "dark", icon: icons.themeDark, "aria-label": t("themeMenu.dark", "Dark") },
      ]}
    />
  );
}
