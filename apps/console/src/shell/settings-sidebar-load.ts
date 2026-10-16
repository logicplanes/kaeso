import type { SettingsSidebar } from "./settings-sidebar";

let loaded: { SettingsSidebar: typeof SettingsSidebar } | undefined;
let loading: Promise<{ SettingsSidebar: typeof SettingsSidebar }> | undefined;

export function loadSettingsSidebar() {
  loading ??= import("./settings-sidebar").then((m) => (loaded = { SettingsSidebar: m.SettingsSidebar }));
  return loading;
}

export const loadedSettingsSidebar = () => loaded;
