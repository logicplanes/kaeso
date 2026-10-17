import type { ReactNode } from "react";
import { expect, onTestFinished } from "vitest";
import { render, waitFor, type RenderResult } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { LinkProvider, ThemeProvider, TooltipProvider } from "@logicplanes/kaeso-ui";
import { apply, load, type Locale } from "~/i18n";
import { I18nProvider } from "~/i18n/provider";
import { queryClient } from "~/query";
import { createAppRouter } from "~/router";
import { RouterLink } from "~/shell/router-link";
import { AppToaster } from "~/shell/toaster";
import { toasterIsMounted } from "~/shell/toast";

export interface Rendered extends RenderResult {
  user: UserEvent;
}

export interface RenderedApp extends Rendered {
  router: ReturnType<typeof createAppRouter>;
}

async function providers(children: ReactNode, locale: Locale): Promise<ReactNode> {
  const initial = await load(locale);
  apply(initial);
  return (
    <QueryClientProvider client={queryClient}>
      <I18nProvider initial={initial}>
        <ThemeProvider>
          <TooltipProvider>
            <LinkProvider component={RouterLink}>{children}</LinkProvider>
          </TooltipProvider>
          <AppToaster />
        </ThemeProvider>
      </I18nProvider>
    </QueryClientProvider>
  );
}

export async function renderApp(path: string, { locale = "en" }: { locale?: Locale } = {}): Promise<RenderedApp> {
  window.history.replaceState(null, "", path);
  const router = createAppRouter();
  onTestFinished(() => router.history.destroy());
  const view = render(await providers(<RouterProvider router={router} />, locale));
  await waitFor(() => expect(router.state.resolvedLocation && router.state.status === "idle").toBe(true));
  await waitFor(() => expect(toasterIsMounted()).toBe(true));
  return { ...view, user: userEvent.setup(), router };
}

export async function renderUi(ui: ReactNode, { path = "/", locale = "en" }: { path?: string; locale?: Locale } = {}): Promise<Rendered> {
  const testRouter = createRouter({
    routeTree: createRootRoute({ component: () => ui }),
    history: createMemoryHistory({ initialEntries: [path] }),
    context: { queryClient },
  });
  await testRouter.load();
  const view = render(await providers(<RouterProvider router={testRouter} />, locale));
  await waitFor(() => expect(toasterIsMounted()).toBe(true));
  return { ...view, user: userEvent.setup() };
}
