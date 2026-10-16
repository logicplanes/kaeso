import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { signedIn, whoAmI } from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { renderApp } from "~/test/render";

const gates = vi.hoisted(() => ({ broken: true }));

vi.mock("~/auth/route-gates-load", async (importOriginal) => {
  const actual = await importOriginal<typeof import("~/auth/route-gates-load")>();
  return {
    loadRouteGates: () => (gates.broken ? Promise.reject(new TypeError("Failed to fetch dynamically imported module")) : actual.loadRouteGates()),
  };
});

describe("when part of the console can't download", () => {
  it("says the gateway isn't answering and opens once the download works", async () => {
    signedIn({ session: whoAmI({ memberships: [] }), memberships: [] });
    const { user } = await renderApp("/");
    expect(await screen.findByRole("heading", { name: "Kaeso isn't answering" })).toBeInTheDocument();
    expect(await notifications().findByText("Can't reach the gateway; retrying")).toBeInTheDocument();
    gates.broken = false;
    await user.click(within(screen.getByRole("main")).getByRole("button", { name: "Retry now" }));
    expect(await screen.findByRole("heading", { name: "Welcome to Kaeso", level: 1 })).toBeInTheDocument();
  });
});
