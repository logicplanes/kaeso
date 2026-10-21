import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import { ORG, signedIn } from "~/test/fixtures";
import { on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PATH = `/orgs/${ORG}`;

const release = (version: string, notes = `## Added\n\n- Everything new in ${version}.`) => ({
  version,
  published_at: "2026-10-15T12:00:00Z",
  prerelease: false,
  notes,
  url: `https://github.com/logicplanes/kaeso/releases/tag/v${version}`,
});

describe("what's new", () => {
  it("draws each release's stored artwork, and one from the version when it has none", async () => {
    signedIn();
    const stored = { ...release("40.5.0"), art: { version: 1, seed: 42, style: "albers", palette: 3 } };
    const unknown = { ...release("40.4.0"), art: { version: 9, seed: 42, style: "glow", palette: 3 } };
    on("GET", "me/whats-new", { current: "40.5.0", seen: "40.3.0", releases: [stored, unknown] });
    const { user } = await renderApp(PATH);
    const dialog = await screen.findByRole("dialog", { name: "What's new in Kaeso" });
    const albers = within(dialog).getByRole("img", { name: "Kaeso 40.5.0" });
    expect(albers.querySelectorAll("circle, path")).toHaveLength(0);
    expect(albers.querySelectorAll("rect").length).toBeGreaterThanOrEqual(5);
    const drawn = albers.innerHTML;
    await user.click(within(dialog).getByRole("button", { name: "Next" }));
    const fallback = within(dialog).getByRole("img", { name: "Kaeso 40.4.0" });
    expect(fallback.innerHTML).not.toBe(drawn);
    expect(fallback.querySelectorAll("rect, circle, path").length).toBeGreaterThan(3);
  });

  it("marks a pre-release as one", async () => {
    signedIn();
    on("GET", "me/whats-new", { current: "40.5.0-rc.1", seen: "40.4.0", releases: [{ ...release("40.5.0-rc.1"), prerelease: true }] });
    await renderApp(PATH);
    const dialog = await screen.findByRole("dialog", { name: "What's new in Kaeso" });
    expect(within(dialog).getByText("Kaeso pre-release")).toBeInTheDocument();
  });

  it("walks through every release since the last visit and records it as seen", async () => {
    signedIn();
    on("GET", "me/whats-new", { current: "40.5.0", seen: "40.3.0", releases: [release("40.5.0"), release("40.4.0")] });
    const { user } = await renderApp(PATH);
    const dialog = await screen.findByRole("dialog", { name: "What's new in Kaeso" });
    expect(within(dialog).getByRole("img", { name: "Kaeso 40.5.0" })).toBeInTheDocument();
    expect(within(dialog).getByText("Published 15 Oct 2026 · 1 of 2 releases")).toBeInTheDocument();
    expect(within(dialog).queryByText("Everything new in 40.5.0.")).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Show improvements" }));
    expect(within(dialog).getByText("Everything new in 40.5.0.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Next" }));
    expect(within(dialog).getByRole("img", { name: "Kaeso 40.4.0" })).toBeInTheDocument();
    expect(within(dialog).queryByText("Everything new in 40.4.0.")).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Back" })).not.toBeInTheDocument();
    expect(requests("POST", "me/whats-new/seen")).toHaveLength(0);
    on("GET", "me/whats-new", { current: "40.5.0", seen: "40.5.0", releases: [] });
    await user.click(within(dialog).getByRole("button", { name: "Continue" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "What's new in Kaeso" })).not.toBeInTheDocument());
    await waitFor(() => expect(requests("POST", "me/whats-new/seen")).toHaveLength(1));
  });

  it("goes away with Escape and counts as seen", async () => {
    signedIn();
    on("GET", "me/whats-new", { current: "40.5.0", seen: "40.4.0", releases: [release("40.5.0")] });
    const { user } = await renderApp(PATH);
    await screen.findByRole("dialog", { name: "What's new in Kaeso" });
    expect(screen.queryByRole("button", { name: "Next" })).not.toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "What's new in Kaeso" })).not.toBeInTheDocument());
    await waitFor(() => expect(requests("POST", "me/whats-new/seen")).toHaveLength(1));
  });

  it("links to GitHub when the install doesn't know a release's notes", async () => {
    signedIn();
    on("GET", "me/whats-new", {
      current: "40.5.0",
      seen: "40.4.0",
      releases: [{ ...release("40.5.0", ""), published_at: null }],
    });
    await renderApp(PATH);
    const dialog = await screen.findByRole("dialog", { name: "What's new in Kaeso" });
    expect(within(dialog).getByText("Now running on this install")).toBeInTheDocument();
    expect(within(dialog).getByText("The notes of this release aren't on this install yet.")).toBeInTheDocument();
    expect(within(dialog).getByRole("link", { name: /Release notes on GitHub/ })).toHaveAttribute(
      "href",
      "https://github.com/logicplanes/kaeso/releases/tag/v40.5.0",
    );
  });

  it("shows nothing on someone's first visit and remembers the version", async () => {
    signedIn();
    on("GET", "me/whats-new", { current: "40.5.0", seen: null, releases: [] });
    await renderApp(PATH);
    await waitFor(() => expect(requests("POST", "me/whats-new/seen")).toHaveLength(1));
    expect(screen.queryByRole("dialog", { name: "What's new in Kaeso" })).not.toBeInTheDocument();
  });
});
