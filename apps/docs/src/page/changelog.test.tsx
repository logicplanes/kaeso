import { afterEach, describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { LinkProvider, ThemeProvider } from "@logicplanes/kaeso-ui";
import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { TextProvider } from "~/i18n/text";
import { createDocsRouter } from "~/router";
import { RouterLink } from "~/shell/router-link";
import { Changelog, releaseDate } from "./changelog";

const repo = "https://github.com/logicplanes/kaeso";

afterEach(() => setFormatLocale("en"));

function app(path: string) {
  render(
    <ThemeProvider>
      <LinkProvider component={RouterLink}>
        <RouterProvider router={createDocsRouter(createMemoryHistory({ initialEntries: [path] }))} />
      </LinkProvider>
    </ThemeProvider>,
  );
}

const release = (version: string) => screen.getByRole("region", { name: version });

describe("the changelog page", () => {
  it("shows every release with its date, its marks and its links, newest first", async () => {
    app("/changelog");
    expect(await screen.findByRole("heading", { level: 1, name: "Changelog" })).toBeInTheDocument();
    expect(screen.getByText("Every change people will notice, release by release.")).toBeInTheDocument();
    expect(screen.queryByText("The changelog is written in English.")).not.toBeInTheDocument();
    expect(screen.getAllByRole("heading", { level: 2 }).map((h) => [h.id, h.textContent])).toEqual([
      ["unreleased", "Unreleased"],
      ["v1.3.0-rc.1", "1.3.0-rc.1"],
      ["v1.2.0", "1.2.0"],
      ["v1.1.1", "1.1.1"],
      ["v1.1.0", "1.1.0"],
      ["v1.0.0", "1.0.0"],
    ]);
    const latest = release("1.2.0");
    expect(within(latest).getByText("Latest")).toBeInTheDocument();
    expect(screen.getAllByText("Latest")).toHaveLength(1);
    expect(within(release("1.3.0-rc.1")).getByText("Pre-release")).toBeInTheDocument();
    expect(within(release("1.1.1")).getByText("Withdrawn")).toBeInTheDocument();
    expect(latest.querySelector("time")).toHaveAttribute("dateTime", "2026-11-15");
    expect(latest.querySelector("time")).toHaveTextContent("15 Nov 2026");
    expect(within(latest).getByRole("link", { name: /Release on GitHub/ })).toHaveAttribute("href", `${repo}/releases/tag/v1.2.0`);
    expect(within(latest).getByRole("link", { name: /Changes since 1\.1\.1/ })).toHaveAttribute("href", `${repo}/compare/v1.1.1...v1.2.0`);
    expect(within(release("1.0.0")).queryByRole("link", { name: /Changes since/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: /Edit on GitHub/ })).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Every release on GitHub/ })).toHaveAttribute("href", `${repo}/releases`);
  });

  it("marks the lines under Unreleased as not released yet", async () => {
    app("/changelog");
    const unreleased = await screen.findByRole("region", { name: "Unreleased" });
    expect(within(unreleased).getByText("Not in a release yet; these changes come with the next one.")).toBeInTheDocument();
    expect(within(unreleased).getByRole("link", { name: /Changes since 1\.3\.0-rc\.1/ })).toHaveAttribute("href", `${repo}/compare/v1.3.0-rc.1...HEAD`);
    expect(unreleased.querySelector("time")).toBeNull();
    expect(within(unreleased).queryByText("Latest")).not.toBeInTheDocument();
  });

  it("draws sections, topics, nested lists and links, with anchors on every heading", async () => {
    app("/changelog");
    const latest = await screen.findByRole("region", { name: "1.2.0" });
    expect(
      within(latest)
        .getAllByRole("heading", { level: 3 })
        .map((h) => [h.id, h.textContent]),
    ).toEqual([
      ["v1.2.0-added", "Added"],
      ["v1.2.0-changed", "Changed"],
      ["v1.2.0-deprecated", "Deprecated"],
      ["v1.2.0-removed", "Removed"],
      ["v1.2.0-security", "Security"],
    ]);
    expect(within(latest).getByRole("heading", { level: 4, name: "Model API" })).toHaveAttribute("id", "v1.2.0-added-model-api");
    const outer = within(latest)
      .getByText(/Aliases that point at a model in another project/)
      .closest("li");
    expect(
      within(outer as HTMLElement)
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["•one alias per project,", "•resolved before the limits are checked."]);
    expect(within(latest).getByRole("link", { name: /the retention/ })).toHaveAttribute("href", "https://docs.kaeso.app/console/request-logs");
    expect(within(release("1.1.0")).getByRole("link", { name: "1.0.0" })).toHaveAttribute("href", "#v1.0.0");
    const toc = screen.getByRole("navigation", { name: "On this page" });
    expect(
      within(toc)
        .getAllByRole("link")
        .map((l) => l.textContent),
    ).toEqual(["Unreleased", "1.3.0-rc.1", "1.2.0", "1.1.1", "1.1.0", "1.0.0"]);
  });

  it("translates its own words in German and marks the English entries as English", async () => {
    app("/de/changelog");
    expect(await screen.findByRole("heading", { level: 1, name: "Änderungen" })).toBeInTheDocument();
    expect(screen.getByText("Die Einträge sind auf Englisch verfasst.")).toBeInTheDocument();
    const latest = release("1.2.0");
    expect(within(latest).getByText("Aktuell")).toBeInTheDocument();
    expect(latest.querySelector("time")).toHaveTextContent("15. Nov. 2026");
    expect(within(latest).getByRole("heading", { level: 3, name: "Hinzugefügt" })).not.toHaveAttribute("lang");
    expect(within(latest).getByRole("heading", { level: 4, name: "Model API" })).toHaveAttribute("lang", "en");
    expect(within(latest).getByText("A filter for aliases on the models page.").closest("[lang]")).toHaveAttribute("lang", "en");
    expect(within(latest).getByRole("link", { name: /Änderungen seit 1\.1\.1/ })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: "Unveröffentlicht" })).toBeInTheDocument();
  });

  it("translates its own words in Chinese", async () => {
    app("/zh-CN/changelog");
    expect(await screen.findByRole("heading", { level: 1, name: "更新日志" })).toBeInTheDocument();
    const latest = release("1.2.0");
    expect(within(latest).getByText("最新")).toBeInTheDocument();
    expect(latest.querySelector("time")).toHaveTextContent("2026年11月15日");
    expect(within(latest).getByRole("heading", { level: 3, name: "新增" })).toBeInTheDocument();
    expect(within(release("1.3.0-rc.1")).getByText("预发布版本")).toBeInTheDocument();
  });

  it("says when there are no releases yet, and keeps a date it can't read as written", () => {
    render(
      <TextProvider locale="en">
        <Changelog changelog={{ releases: [] }} />
      </TextProvider>,
    );
    expect(screen.getByText("No releases yet.")).toBeInTheDocument();
    expect(releaseDate("2026-02-30")).toBe("2026-02-30");
  });
});
