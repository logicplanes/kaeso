import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, signedIn } from "~/test/fixtures";
import { renderApp } from "~/test/render";

const PLACEHOLDER = "Search pages, actions, keys, models and members";

describe("the command palette before it has loaded", () => {
  it("keeps what is typed while it loads, leaves out shortcuts and starts empty the next time", async () => {
    signedIn();
    const { user } = await renderApp(`/orgs/${ORG}/projects`);
    await screen.findByRole("navigation", { name: "Console" });
    fireEvent.keyDown(window, { key: "k", metaKey: true });
    fireEvent.keyDown(window, { key: "k" });
    fireEvent.keyDown(window, { key: "e" });
    fireEvent.keyDown(window, { key: "x" });
    fireEvent.keyDown(window, { key: "Backspace" });
    fireEvent.keyDown(window, { key: "Shift" });
    fireEvent.keyDown(window, { key: "y", altKey: true });
    fireEvent.keyDown(window, { key: "y", ctrlKey: true });
    fireEvent.keyDown(window, { key: "y", metaKey: true });
    fireEvent.keyDown(window, { key: "y" });
    const input = await screen.findByPlaceholderText(PLACEHOLDER);
    await waitFor(() => expect(input).toHaveValue("key"));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByPlaceholderText(PLACEHOLDER)).not.toBeInTheDocument());
    await user.keyboard("{Meta>}k{/Meta}");
    expect(await screen.findByPlaceholderText(PLACEHOLDER)).toHaveValue("");
  });
});
