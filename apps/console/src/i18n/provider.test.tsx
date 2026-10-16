import { screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { signedIn, whoAmI } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = "/settings/account/appearance";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("the console's language", () => {
  it("follows the language saved to the account", async () => {
    signedIn({ session: whoAmI({ locale: "de" }) });
    await renderApp(PATH);
    expect(await screen.findByRole("heading", { name: "Darstellung", level: 1 })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("de");
    expect(window.localStorage.getItem("kaeso:locale")).toBe("de");
  });

  it("loads the page's data again in the new language", async () => {
    signedIn();
    on("PATCH", "me", ({ body }: { body: Parameters<typeof me>[0] }) => {
      on("GET", "auth/session", whoAmI(body));
      return me(body);
    });
    const { user } = await renderApp(PATH);
    await user.click(await screen.findByRole("combobox", { name: "Language" }));
    const before = requests("GET", "me/memberships").length;
    await user.click(await screen.findByRole("option", { name: "简体中文" }));
    await waitFor(() => expect(requests("GET", "me/memberships").length).toBeGreaterThan(before));
    await waitFor(() => expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("外观"));
  });

  it("follows the browser when the person leaves it automatic", async () => {
    signedIn();
    const languages = vi.spyOn(navigator, "languages", "get").mockReturnValue(["en-US"]);
    await renderApp(PATH);
    await screen.findByRole("heading", { name: "Appearance", level: 1 });
    languages.mockReturnValue(["de-AT", "en"]);
    window.dispatchEvent(new Event("languagechange"));
    expect(await screen.findByRole("heading", { name: "Darstellung", level: 1 })).toBeInTheDocument();
  });

  it("ends in the language the browser changed to last", async () => {
    signedIn();
    const languages = vi.spyOn(navigator, "languages", "get").mockReturnValue(["en-US"]);
    await renderApp(PATH);
    await screen.findByRole("heading", { name: "Appearance", level: 1 });
    languages.mockReturnValue(["de-DE"]);
    window.dispatchEvent(new Event("languagechange"));
    languages.mockReturnValue(["zh-CN"]);
    window.dispatchEvent(new Event("languagechange"));
    expect(await screen.findByRole("heading", { name: "外观", level: 1 })).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("zh-CN");
  });

  it("keeps a language the person chose when the browser's changes", async () => {
    signedIn({ session: whoAmI({ locale: "en" }) });
    const languages = vi.spyOn(navigator, "languages", "get").mockReturnValue(["en-US"]);
    await renderApp(PATH);
    await screen.findByRole("heading", { name: "Appearance", level: 1 });
    await waitFor(() => expect(window.localStorage.getItem("kaeso:locale")).toBe("en"));
    languages.mockReturnValue(["de-DE"]);
    window.dispatchEvent(new Event("languagechange"));
    await screen.findByRole("combobox", { name: "Language" });
    expect(screen.queryByRole("heading", { name: "Darstellung" })).not.toBeInTheDocument();
    expect(document.documentElement.lang).toBe("en");
  });
});
