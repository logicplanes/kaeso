import { forwardRef } from "react";
import { useTheme, type ThemePreference } from "../../theme/theme-provider.js";
import { useUiText } from "../../i18n/context.js";
import { Select } from "../select/select.js";

export const KAESO_THEME_SELECT_VARIANTS = {
  appearance: {
    ghost: { classes: "", description: "A quiet pill for sidebars and toolbars (default)" },
    default: { classes: "", description: "A raised pill with a ring" },
  },
  size: {
    sm: { classes: "", description: "32px (default)" },
    md: { classes: "", description: "36px" },
  },
} as const;
export const KAESO_THEME_SELECT_DEFAULT_VARIANTS = { appearance: "ghost", size: "sm" } as const;

const PREFERENCES: readonly ThemePreference[] = ["system", "light", "dark"];
const isPreference = (value: string): value is ThemePreference => (PREFERENCES as readonly string[]).includes(value);

export interface ThemeSelectProps {
  appearance?: keyof typeof KAESO_THEME_SELECT_VARIANTS.appearance | undefined;
  size?: keyof typeof KAESO_THEME_SELECT_VARIANTS.size | undefined;
  fullWidth?: boolean | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

export const ThemeSelect = forwardRef<HTMLButtonElement, ThemeSelectProps>(function ThemeSelect(
  { appearance = "ghost", size = "sm", fullWidth, className, "data-force": force },
  ref,
) {
  const { theme, setTheme } = useTheme();
  const t = useUiText();
  return (
    <Select
      ref={ref}
      aria-label={t("themeMenu.theme", "Theme")}
      appearance={appearance}
      size={size}
      fullWidth={fullWidth}
      className={className}
      data-force={force}
      value={theme}
      onValueChange={(value) => {
        if (isPreference(value)) setTheme(value);
      }}
      options={[
        { value: "system", label: t("themeMenu.system", "System") },
        { value: "light", label: t("themeMenu.light", "Light") },
        { value: "dark", label: t("themeMenu.dark", "Dark") },
      ]}
    />
  );
});
ThemeSelect.displayName = "ThemeSelect";
