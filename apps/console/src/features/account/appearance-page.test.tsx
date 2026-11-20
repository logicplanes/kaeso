import { describe, expect, it } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { signedIn, whoAmI } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { apiError, on, requests } from "~/test/network";
import { notifications } from "~/test/fixtures/org";
import { renderApp } from "~/test/render";

const PATH = "/settings/account/appearance";

describe("appearance", () => {
  it("switches the theme in this browser", async () => {
    signedIn();
    const { user } = await renderApp(PATH);
    const theme = await screen.findByRole("combobox", { name: "Theme" });
    expect(theme).toHaveTextContent("System");
    await user.click(theme);
    await user.click(await screen.findByRole("option", { name: "Dark" }));
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Theme" })).toHaveTextContent("Dark"));
    expect(requests("PATCH", "me")).toHaveLength(0);
  });

  it("saves the language to the account and shows the page in it", async () => {
    signedIn();
    on("PATCH", "me", ({ body }: { body: Parameters<typeof me>[0] }) => {
      on("GET", "auth/session", whoAmI(body));
      return me(body);
    });
    const { user } = await renderApp(PATH);
    const language = await screen.findByRole("combobox", { name: "Language" });
    expect(language).toHaveTextContent("Automatic (English)");
    await user.click(language);
    await user.click(await screen.findByRole("option", { name: "Deutsch" }));
    await waitFor(() => expect(screen.getByRole("heading", { name: "Darstellung", level: 1 })).toBeInTheDocument());
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Sprache" })).toHaveTextContent("Deutsch"));
    expect(requests("PATCH", "me").map((r) => r.body)).toEqual([{ locale: "de" }]);
  });

  it("shows the console in the language saved to the account", async () => {
    signedIn({ session: whoAmI({ locale: "zh-CN" }) });
    await renderApp(PATH);
    await waitFor(() => expect(screen.getByRole("heading", { name: "外观", level: 1 })).toBeInTheDocument());
    await waitFor(() => expect(screen.getByRole("combobox", { name: "语言" })).toHaveTextContent("简体中文"));
  });

  it("keeps the language when saving it is refused", async () => {
    signedIn();
    on("PATCH", "me", apiError(422, "invalid_request", "That language isn't supported."));
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("combobox", { name: "Language" }));
    await user.click(await screen.findByRole("option", { name: "English" }));
    expect((await notifications().findAllByText("That language isn't supported.")).length).toBeGreaterThan(0);
    expect(screen.getByRole("combobox", { name: "Language" })).toHaveTextContent("Automatic (English)");
  });
});
