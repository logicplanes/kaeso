import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, PROJECT, signedIn } from "~/test/fixtures";
import { renderApp } from "~/test/render";

describe("a page that doesn't exist", () => {
  it("links back to the last place the person worked in", async () => {
    signedIn();
    window.localStorage.setItem("kaeso:last", JSON.stringify({ org: ORG, project: PROJECT }));
    await renderApp("/no/such/page");
    expect(await screen.findByRole("heading", { name: "Not found", level: 1 })).toBeInTheDocument();
    expect(document.title).toBe("Not found · Kaeso");
    expect(screen.getByText("This page doesn't exist.")).toBeInTheDocument();
    expect(screen.getByText("The link may be old, or the page moved.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Go to the overview" })).toHaveAttribute("href", `/orgs/${ORG}/projects/${PROJECT}`);
  });

  it("links to the start without a last place", async () => {
    signedIn();
    await renderApp("/no/such/page");
    expect(await screen.findByRole("link", { name: "Go to the overview" })).toHaveAttribute("href", "/");
  });
});
