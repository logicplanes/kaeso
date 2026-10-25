import { forwardRef } from "react";
import { icons } from "../../icons/vocabulary.js";
import { useTheme, type ThemePreference } from "../../theme/theme-provider.js";
import { IconButton } from "../icon-button/icon-button.js";
import { Menu } from "../menu/menu.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_THEME_MENU_VARIANTS = {
  variant: {
    ghost: { classes: "", description: "A ghost circle (default)" },
    secondary: { classes: "", description: "A raised circle" },
  },
} as const;
export const KAESO_THEME_MENU_DEFAULT_VARIANTS = { variant: "ghost" } as const;

export interface ThemeMenuProps {
  variant?: keyof typeof KAESO_THEME_MENU_VARIANTS.variant | undefined;
  size?: "sm" | "md" | undefined;
  "data-force"?: string | undefined;
}

export const ThemeMenu = forwardRef<HTMLButtonElement, ThemeMenuProps>(function ThemeMenu({ variant = "ghost", size = "sm", "data-force": force }, ref) {
  const { theme, resolvedTheme, setTheme } = useTheme();
  const t = useUiText();
  return (
    <Menu>
      <Menu.Trigger>
        <IconButton
          ref={ref}
          icon={resolvedTheme === "dark" ? icons.themeDark : icons.themeLight}
          aria-label={t("themeMenu.theme", "Theme")}
          variant={variant}
          size={size}
          data-force={force}
        />
      </Menu.Trigger>
      <Menu.Content align="end" className="min-w-180">
        <Menu.RadioGroup value={theme} onValueChange={(v) => setTheme(v as ThemePreference)}>
          <Menu.RadioItem value="system" icon={icons.themeSystem}>
            {t("themeMenu.system", "System")}
          </Menu.RadioItem>
          <Menu.RadioItem value="light" icon={icons.themeLight}>
            {t("themeMenu.light", "Light")}
          </Menu.RadioItem>
          <Menu.RadioItem value="dark" icon={icons.themeDark}>
            {t("themeMenu.dark", "Dark")}
          </Menu.RadioItem>
        </Menu.RadioGroup>
      </Menu.Content>
    </Menu>
  );
});
ThemeMenu.displayName = "ThemeMenu";
