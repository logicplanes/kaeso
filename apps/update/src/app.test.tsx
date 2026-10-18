import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App, COLLAPSE_AFTER, Root } from "./app";
import { setLocale } from "./i18n";
import type { Release } from "./releases";

const latest: Release = {
  version: "3.1.0",
  prerelease: false,
  publishedAt: "2027-02-16T14:05:00Z",
  notes: "## Added\n\n- Organizations can require single sign-on for every member.\n\n## Fixed\n\n- Streaming responses keep the last token count.",
  url: "https://github.com/logicplanes/kaeso/releases/tag/v3.1.0",
};

const rc: Release = {
  version: "3.2.0-rc.1",
  prerelease: true,
  publishedAt: "2027-03-02T09:30:00Z",
  notes: "## Added\n\n- Projects can cap the cost of a single request.",
  url: "https://github.com/logicplanes/kaeso/releases/tag/v3.2.0-rc.1",
};

const long: Release = {
  version: "3.0.0",
  prerelease: false,
  publishedAt: null,
  notes: Array.from({ length: 30 }, (_, i) => `- Change number ${i + 1} to how the gateway routes requests.`).join("\n"),
  url: "https://github.com/logicplanes/kaeso/releases/tag/v3.0.0",
};

const bare: Release = {
  version: "2.9.0",
  prerelease: false,
  publishedAt: "2026-12-01T10:00:00Z",
  notes: "",
  url: "https://github.com/logicplanes/kaeso/releases/tag/v2.9.0",
};

afterEach(() => setLocale("en"));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("the releases page", () => {
  it("shows the wordmark and the page's title", async () => {
    render(<App load={async () => [latest]} />);
    expect(screen.getByRole("img", { name: "Kaeso" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Releases" })).toBeInTheDocument();
    expect(screen.getByText("Every Kaeso release, its notes and how to upgrade.")).toBeInTheDocument();
    await screen.findByRole("heading", { level: 2, name: "Kaeso 3.1.0" });
  });

  it("shows the newest stable release first, with its artwork, date, notes and links", async () => {
    render(<App load={async () => [rc, latest, long]} />);
    const hero = await screen.findByRole("region", { name: "Kaeso 3.1.0" });
    expect(within(hero).getByRole("img", { name: "Kaeso 3.1.0" })).toBeInTheDocument();
    expect(within(hero).getByText("Latest release")).toBeInTheDocument();
    expect(within(hero).getByText("Published 16 Feb 2027")).toBeInTheDocument();
    expect(within(hero).getByRole("heading", { level: 3, name: "Added" })).toBeInTheDocument();
    expect(within(hero).getByText("Organizations can require single sign-on for every member.")).toBeInTheDocument();
    expect(within(hero).getByRole("link", { name: /Release notes on GitHub/ })).toHaveAttribute("href", latest.url);
    expect(within(hero).getByRole("link", { name: /Upgrade guide/ })).toHaveAttribute("href", "https://docs.kaeso.app/self-hosting/upgrades");
  });

  it("lists every other release newest first, pre-releases marked, long notes collapsed", async () => {
    const user = userEvent.setup();
    render(<App load={async () => [rc, latest, long, bare]} />);
    const others = await screen.findByRole("region", { name: "Other releases" });
    const items = within(others)
      .getAllByRole("listitem")
      .filter((li) => li.parentElement?.tagName === "OL");
    const [first, second, third] = items as [HTMLElement, HTMLElement, HTMLElement];
    expect(items.map((li) => within(li).getByRole("heading", { level: 3 }).textContent)).toEqual(["Kaeso 3.2.0-rc.1", "Kaeso 3.0.0", "Kaeso 2.9.0"]);
    expect(within(first).getByRole("img", { name: "Kaeso 3.2.0-rc.1" })).toBeInTheDocument();
    expect(within(first).getByText("Pre-release")).toBeInTheDocument();
    expect(within(first).getByText("Published 2 Mar 2027")).toBeInTheDocument();
    expect(within(first).getByRole("heading", { level: 4, name: "Added" })).toBeInTheDocument();
    expect(within(second).queryByText("Pre-release")).not.toBeInTheDocument();
    expect(within(second).queryByText(/^Published/)).not.toBeInTheDocument();
    expect(long.notes.length).toBeGreaterThan(COLLAPSE_AFTER);
    expect(within(second).queryByText("Change number 30 to how the gateway routes requests.")).not.toBeInTheDocument();
    await user.click(within(second).getByRole("button", { name: `Show all ${long.notes.length.toLocaleString("en")} characters` }));
    expect(within(second).getByText("Change number 30 to how the gateway routes requests.")).toBeInTheDocument();
    expect(within(third).getByText("This release has no notes.")).toBeInTheDocument();
    expect(within(third).getByRole("link", { name: /Release notes on GitHub/ })).toHaveAttribute("href", bare.url);
  });

  it("shows the newest pre-release when there is no stable release yet", async () => {
    render(<App load={async () => [rc]} />);
    const hero = await screen.findByRole("region", { name: "Kaeso 3.2.0-rc.1" });
    expect(within(hero).getByText("Latest pre-release")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Other releases" })).not.toBeInTheDocument();
  });

  it("says that a release has no notes and leaves out a missing date", async () => {
    render(<App load={async () => [{ ...bare, publishedAt: null }]} />);
    const hero = await screen.findByRole("region", { name: "Kaeso 2.9.0" });
    expect(within(hero).getByText("This release has no notes.")).toBeInTheDocument();
    expect(within(hero).queryByText(/^Published/)).not.toBeInTheDocument();
  });

  it("holds the hero's place while the feed loads", async () => {
    const pending = deferred<Release[]>();
    render(<App load={() => pending.promise} />);
    expect(screen.getByRole("region", { name: "Loading releases" })).toHaveAttribute("aria-busy", "true");
    await act(async () => pending.resolve([latest]));
    expect(screen.queryByRole("region", { name: "Loading releases" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: "Kaeso 3.1.0" })).toBeInTheDocument();
  });

  it("says when there are no releases yet", async () => {
    render(<App load={async () => []} />);
    expect(await screen.findByText("No releases yet")).toBeInTheDocument();
    expect(screen.getByText("Published releases appear here with their notes.")).toBeInTheDocument();
  });

  it("says when the feed didn't load and tries again", async () => {
    const user = userEvent.setup();
    const load = vi.fn<(signal: AbortSignal) => Promise<Release[]>>().mockRejectedValueOnce(new Error("offline")).mockResolvedValueOnce([latest]);
    render(<App load={load} />);
    expect(await screen.findByText("Releases didn't load")).toBeInTheDocument();
    expect(screen.getByText("Check your connection, then try again.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("heading", { level: 2, name: "Kaeso 3.1.0" })).toBeInTheDocument();
    expect(load).toHaveBeenCalledTimes(2);
  });

  it("ignores an answer that arrives after the page is gone", async () => {
    const ok = deferred<Release[]>();
    const failed = deferred<Release[]>();
    const first = render(<App load={() => ok.promise} />);
    first.unmount();
    const second = render(<App load={() => failed.promise} />);
    second.unmount();
    await act(async () => {
      ok.resolve([latest]);
      failed.reject(new Error("offline"));
    });
    expect(screen.queryByText("Kaeso 3.1.0")).not.toBeInTheDocument();
  });

  it("speaks German with the library in the same language", async () => {
    setLocale("de");
    render(<Root locale="de" ui={{ "markdown.showAllCharacters": "Alle {count} Zeichen anzeigen" }} load={async () => [rc, latest, long]} />);
    expect(screen.getByRole("heading", { level: 1, name: "Releases" })).toBeInTheDocument();
    const hero = await screen.findByRole("region", { name: "Kaeso 3.1.0" });
    expect(within(hero).getByText("Neuestes Release")).toBeInTheDocument();
    expect(within(hero).getByText("Veröffentlicht am 16. Feb. 2027")).toBeInTheDocument();
    expect(within(hero).getByRole("link", { name: /Upgrade-Anleitung/ })).toHaveAttribute("href", "https://docs.kaeso.app/de/self-hosting/upgrades");
    expect(screen.getByRole("heading", { level: 2, name: "Weitere Releases" })).toBeInTheDocument();
    expect(screen.getByText("Vorabversion")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: `Alle ${long.notes.length.toLocaleString("de")} Zeichen anzeigen` })).toBeInTheDocument();
  });

  it("renders with the default feed loader", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify([]), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);
    try {
      render(<Root locale="en" ui={{}} />);
      expect(await screen.findByText("No releases yet")).toBeInTheDocument();
      expect(fetchMock).toHaveBeenCalledWith("/v1/history.json", expect.objectContaining({ signal: expect.any(AbortSignal) }));
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
