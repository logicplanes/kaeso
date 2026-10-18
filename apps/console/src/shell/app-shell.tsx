import { Suspense, lazy, useEffect, useState } from "react";
import { Outlet, useLocation, useParams } from "@tanstack/react-router";
import { AppFrame, Sidebar, TooltipProvider } from "@logicplanes/kaeso-ui";
import { RecentSignInHost } from "~/auth/recent-sign-in";
import { useMemberships } from "~/auth/session";
import { t } from "~/i18n";
import { AppSidebar } from "./app-sidebar";
import { CommandPaletteHost } from "./palette";
import { CreateActionProvider } from "./create-action";
import "./early-keys";
import { readLast } from "./last";
import { WhatsNewHost } from "./whats-new-host";
import { orgPath, placePath } from "./pages";
import { loadSettingsSidebar, loadedSettingsSidebar } from "./settings-sidebar-load";

const ShellListeners = lazy(() => import("./listeners"));

if (location.pathname.startsWith("/settings")) void loadSettingsSidebar();

function SettingsSidebarSlot() {
  const [loaded, setLoaded] = useState(loadedSettingsSidebar);
  useEffect(() => {
    if (!loaded) void loadSettingsSidebar().then(setLoaded);
  }, [loaded]);
  return loaded ? <loaded.SettingsSidebar /> : <Sidebar aria-label={t("common.settings", "Settings")} />;
}

export function AppShell() {
  const settings = useLocation({ select: (l) => l.pathname.startsWith("/settings") });
  const params = useParams({ strict: false });
  const memberships = useMemberships();
  const last = readLast();
  const org = params.org ?? last?.org ?? memberships[0]?.org_id;
  const brand = settings
    ? { brandHref: last ? placePath(last) : org ? orgPath(org) : "/", brandLabel: t("shell.backToConsole", "Back to the console") }
    : { brandHref: org ? orgPath(org) : "/", brandLabel: t("shell.home", "Kaeso home") };
  useEffect(() => {
    const timer = setTimeout(() => void loadSettingsSidebar(), 1500);
    return () => clearTimeout(timer);
  }, []);
  return (
    <TooltipProvider>
      <CreateActionProvider>
        <Sidebar.Provider storageKey="kaeso:sidebar">
          <AppFrame sidebar={settings ? <SettingsSidebarSlot /> : <AppSidebar />} {...brand}>
            <Outlet />
          </AppFrame>
          <Suspense fallback={null}>
            <ShellListeners />
          </Suspense>
        </Sidebar.Provider>
        <CommandPaletteHost />
        <RecentSignInHost />
        <WhatsNewHost />
      </CreateActionProvider>
    </TooltipProvider>
  );
}
