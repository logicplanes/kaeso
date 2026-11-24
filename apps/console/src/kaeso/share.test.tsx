import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG } from "~/test/fixtures";
import { Share } from "./share";

describe("the share button", () => {
  it("stands disabled in its place while its code loads, so the header's other actions don't move when it arrives", () => {
    // A lazy part always suspends on its first render, so this first frame is the one a slow load shows for longer.
    render(<Share org={ORG} target={{ kind: "analytics", from: "2026-10-01", to: "2026-10-14", costs: true }} />);
    expect(screen.getByRole("button", { name: "Share" })).toBeDisabled();
  });
});
