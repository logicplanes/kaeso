import { describe, expect, it } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { exchangeRates, signedIn, whoAmI } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { apiError, on, requests } from "~/test/network";
import { hoverReason, notifications } from "~/test/fixtures/org";
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

describe("display currency", () => {
  it("saves the currency spend also shows in to the account", async () => {
    signedIn();
    let mine = me();
    on("GET", "me", () => mine);
    on("PATCH", "me", ({ body }: { body: Parameters<typeof me>[0] }) => (mine = me(body)));
    const { user } = await renderApp(PATH);

    const currency = await screen.findByRole("combobox", { name: "Currency" });
    await waitFor(() => expect(currency).toHaveTextContent("USD"));
    await user.click(currency);
    expect(await screen.findByRole("option", { name: /^USD.*US dollars only/ })).toBeInTheDocument();
    await user.click(await screen.findByRole("option", { name: /^EUR.*Euro/ }));
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Currency" })).toHaveTextContent("EUR"));
    expect(requests("PATCH", "me").map((r) => r.body)).toEqual([{ display_currency: "EUR" }]);
  });

  it("keeps the currency when saving it is refused", async () => {
    signedIn();
    on(
      "PATCH",
      "me",
      apiError(400, "invalid_request", "Kaeso has no exchange rate for EUR; use a currency from `exchange-rates`.", { param: "display_currency" }),
    );
    const { user } = await renderApp(PATH);

    await user.click(await screen.findByRole("combobox", { name: "Currency" }));
    await user.click(await screen.findByRole("option", { name: /^EUR/ }));
    expect(await notifications().findByText(/^Kaeso has no exchange rate for EUR/)).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Currency" })).toHaveTextContent("USD"));
  });

  it("offers only dollars until the install has exchange rates, and says why", async () => {
    signedIn();
    on("GET", "me/exchange-rates", exchangeRates({}, true));
    const { user } = await renderApp(PATH);

    const only = await screen.findByRole("button", { name: "USD" });
    expect(only).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, only, "No exchange rates yet: Kaeso fetches the ECB's once a working day");
  });
});
