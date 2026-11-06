import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, signedIn, whoAmI } from "~/test/fixtures";
import { me } from "~/test/fixtures/instance-account";
import { on } from "~/test/network";
import { renderApp } from "~/test/render";

function seen(el: Element | null): string | undefined {
  const copy = el?.cloneNode(true) as Element | undefined;
  copy?.querySelectorAll(".sr-only").forEach((hidden) => hidden.remove());
  return copy?.textContent?.replace(/\u2060/g, "");
}

async function notice(name: string) {
  const link = await screen.findByRole("link", { name: new RegExp(`^${name} \\(`) });
  return { link, text: seen(link.closest('[role="status"]')) };
}

describe("a demo install", () => {
  it("says on the organization's pages that it's a read-only demo and links to the installation guide", async () => {
    signedIn({ session: whoAmI({ demo: true }) });
    await renderApp(`/orgs/${ORG}`);
    const { link, text } = await notice("install Kaeso");
    expect(text).toBe("This is a read-only demo with sample data; install Kaeso to connect your own providers.");
    expect(link).toHaveAttribute("href", "https://docs.kaeso.app/self-hosting/installation");
    expect(await screen.findByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
  });

  it("says so in settings too", async () => {
    signedIn({ session: whoAmI({ demo: true }) });
    on("GET", "me", me());
    await renderApp("/settings/account/profile");
    expect((await notice("install Kaeso")).text).toBe("This is a read-only demo with sample data; install Kaeso to connect your own providers.");
  });

  it("links to the guide in the console's language", async () => {
    signedIn({ session: whoAmI({ demo: true }) });
    await renderApp(`/orgs/${ORG}`, { locale: "de" });
    const { link, text } = await notice("installieren Sie Kaeso");
    expect(text).toBe("Dies ist eine schreibgeschützte Demo mit Beispieldaten; installieren Sie Kaeso, um eigene Anbieter zu verbinden.");
    expect(link).toHaveAttribute("href", "https://docs.kaeso.app/de/self-hosting/installation");
  });
});

describe("an install that isn't a demo", () => {
  it("shows no demo notice", async () => {
    signedIn();
    await renderApp(`/orgs/${ORG}`);
    expect(await screen.findByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
    expect(screen.queryByText(/read-only demo/)).not.toBeInTheDocument();
  });
});
