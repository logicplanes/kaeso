import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Timeline } from "./timeline.js";

describe("Timeline", () => {
  it("says a toned event's tone in words, keeps the machine time and drops the raw title attribute", () => {
    render(
      <Timeline
        events={[
          { id: "a", title: "Key revoked", time: "09:40", datetime: "2026-10-12T09:40:00Z", tone: "danger" },
          { id: "b", title: "Key created", time: "08:12" },
        ]}
      />,
    );
    expect(screen.getAllByRole("listitem")[0]).toHaveTextContent("Critical: Key revoked");
    expect(screen.getAllByRole("listitem")[1]).toHaveTextContent(/^Key created/);
    const time = screen.getByText("09:40");
    expect(time).toHaveAttribute("datetime", "2026-10-12T09:40:00Z");
    expect(time).not.toHaveAttribute("title");
    expect(time).toHaveClass("text-fg-secondary");
  });
  it("draws its spine in the line color, which survives forced colours", () => {
    const { container } = render(
      <Timeline
        events={[
          { id: "a", title: "One", time: "1" },
          { id: "b", title: "Two", time: "2" },
        ]}
      />,
    );
    expect(container.querySelector(".bg-line")).toHaveClass("forced-line");
  });
});
