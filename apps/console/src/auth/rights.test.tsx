import { StrictMode } from "react";
import { describe, expect, it, onTestFinished } from "vitest";
import { render, renderHook, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import { LinkProvider, ThemeProvider } from "@logicplanes/kaeso-ui";
import { ORG, PROJECT, membership, signedIn, whoAmI } from "~/test/fixtures";
import { apply, load } from "~/i18n";
import { I18nProvider } from "~/i18n/provider";
import { queryClient } from "~/query";
import { createAppRouter } from "~/router";
import { RouterLink } from "~/shell/router-link";
import { AppToaster } from "~/shell/toaster";
import { loadedProjectRights } from "./rights";
import { loadedOrgRights, projectRightsQuery, sessionQuery, useOrgRights } from "./session";

const notifications = async () => within(await screen.findByRole("region", { name: "Notifications" }));

async function renderStrictApp(path: string) {
  window.history.replaceState(null, "", path);
  const router = createAppRouter();
  onTestFinished(() => router.history.destroy());
  const initial = await load("en");
  apply(initial);
  render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <I18nProvider initial={initial}>
          <ThemeProvider>
            <LinkProvider component={RouterLink}>
              <RouterProvider router={router} />
            </LinkProvider>
            <AppToaster />
          </ThemeProvider>
        </I18nProvider>
      </QueryClientProvider>
    </StrictMode>,
  );
  return userEvent.setup();
}

describe("an organization the person doesn't belong to, in strict mode", () => {
  it("is checked once and goes home with one notice", async () => {
    signedIn();
    await renderStrictApp("/orgs/org_contoso");
    expect(await (await notifications()).findByText("That organization doesn't exist or you can't access it")).toBeInTheDocument();
    await waitFor(() => expect(window.location.pathname).toMatch(new RegExp(`^/orgs/${ORG}`)));
    expect((await notifications()).getAllByText("That organization doesn't exist or you can't access it")).toHaveLength(1);
  });
});

describe("an organization's rights", () => {
  it("stay the same set while the session doesn't change", () => {
    queryClient.setQueryData(sessionQuery.queryKey, whoAmI());
    const { result, rerender } = renderHook(() => useOrgRights(ORG), {
      wrapper: ({ children }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>,
    });
    const first = result.current;
    rerender();
    expect(result.current).toBe(first);
    expect(first.has("org.view")).toBe(true);
  });
});

describe("the rights a route's loader reads", () => {
  it("are none before the session is loaded", () => {
    expect(loadedOrgRights(queryClient, ORG).size).toBe(0);
    expect(loadedProjectRights(queryClient, ORG, PROJECT).size).toBe(0);
  });

  it("keep only what a read-only organization leaves in a project", () => {
    const session = whoAmI({ memberships: [membership({ org_state: "scheduled_for_deletion" })] });
    queryClient.setQueryData(sessionQuery.queryKey, session);
    queryClient.setQueryData(projectRightsQuery(session, ORG, PROJECT).queryKey, { rights: new Set(["models.view", "models.manage"]), member: true });
    expect([...loadedProjectRights(queryClient, ORG, PROJECT)]).toEqual(["models.view"]);
    expect(loadedOrgRights(queryClient, ORG).has("org.view")).toBe(true);
  });
});
