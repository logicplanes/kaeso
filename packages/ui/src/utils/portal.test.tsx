import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { PortalContainer } from "./portal-context";
import { Dialog } from "../components/dialog/dialog";

describe("PortalContainer", () => {
  it("receives overlays that are open on first render", async () => {
    const { container } = render(
      <div data-theme="dark">
        <PortalContainer>
          <Dialog defaultOpen title="Key created">
            <p>Shown once</p>
          </Dialog>
        </PortalContainer>
      </div>,
    );
    const dialog = await screen.findByRole("dialog");
    expect(container.querySelector("[data-portal-container]")?.contains(dialog)).toBe(true);
    expect(dialog.closest("[data-theme]")).toHaveAttribute("data-theme", "dark");
  });
});
