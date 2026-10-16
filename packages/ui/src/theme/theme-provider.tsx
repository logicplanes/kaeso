import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

export type ThemePreference = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

export const THEME_STORAGE_KEY = "kaeso:theme";

const modalitySource = `if(!window.__kModality){window.__kModality=1;document.addEventListener("pointerdown",function(){d.setAttribute("data-k-pointer","")},true);document.addEventListener("keydown",function(e){if(!e.metaKey&&!e.ctrlKey&&!e.altKey){d.removeAttribute("data-k-pointer")}},true)}`;

export function installFocusModality(): void {
  if (typeof document === "undefined") return;
  const w = window as unknown as { __kModality?: number };
  if (w.__kModality) return;
  w.__kModality = 1;
  const d = document.documentElement;
  document.addEventListener("pointerdown", () => d.setAttribute("data-k-pointer", ""), true);
  document.addEventListener(
    "keydown",
    (e) => {
      if (!e.metaKey && !e.ctrlKey && !e.altKey) d.removeAttribute("data-k-pointer");
    },
    true,
  );
}

export const themeScript = `(function(){var d=document.documentElement;try{var t=localStorage.getItem("${THEME_STORAGE_KEY}");if(t==="light"||t==="dark"){d.setAttribute("data-theme",t)}}catch(e){}${modalitySource}})();`;

interface ThemeContextValue {
  theme: ThemePreference;
  resolvedTheme: ResolvedTheme;
  setTheme: (theme: ThemePreference) => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

function readStored(fallback: ThemePreference): ThemePreference {
  try {
    const v = localStorage.getItem(THEME_STORAGE_KEY);
    return v === "light" || v === "dark" ? v : fallback;
  } catch {
    return fallback;
  }
}

function systemTheme(): ResolvedTheme {
  if (typeof window === "undefined" || !window.matchMedia) return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export interface ThemeProviderProps {
  children: ReactNode;
  defaultTheme?: ThemePreference | undefined;
  target?: (() => HTMLElement) | undefined;
}

export function ThemeProvider({ children, defaultTheme = "system", target }: ThemeProviderProps) {
  const [chosen, setThemeState] = useState<ThemePreference | null>(null);
  const [system, setSystem] = useState<ResolvedTheme>("light");
  const theme = chosen ?? defaultTheme;

  useEffect(() => {
    installFocusModality();
    setThemeState(readStored(defaultTheme));
    setSystem(systemTheme());
    const mq = window.matchMedia?.("(prefers-color-scheme: dark)");
    const onChange = () => setSystem(mq.matches ? "dark" : "light");
    mq?.addEventListener("change", onChange);
    return () => mq?.removeEventListener("change", onChange);
  }, [defaultTheme]);

  useEffect(() => {
    if (chosen === null) return;
    const el = target ? target() : document.documentElement;
    if (chosen === "system") el.removeAttribute("data-theme");
    else el.setAttribute("data-theme", chosen);
  }, [chosen, target]);

  const setTheme = useCallback((next: ThemePreference) => {
    setThemeState(next);
    try {
      if (next === "system") localStorage.removeItem(THEME_STORAGE_KEY);
      else localStorage.setItem(THEME_STORAGE_KEY, next);
    } catch {}
  }, []);

  const value = useMemo<ThemeContextValue>(() => ({ theme, resolvedTheme: theme === "system" ? system : theme, setTheme }), [theme, system, setTheme]);
  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
ThemeProvider.displayName = "ThemeProvider";

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) {
    return { theme: "system", resolvedTheme: "light", setTheme: () => undefined };
  }
  return ctx;
}
