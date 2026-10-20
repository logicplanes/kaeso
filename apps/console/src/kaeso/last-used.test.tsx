import { screen } from "@testing-library/react";
import { act } from "react";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { describe, expect, it } from "vitest";
import { renderUi } from "~/test/render";
import { LastUsed } from "./last-used";

describe("LastUsed", () => {
  it("says what the time is, and gives the exact time on hover", async () => {
    const at = new Date(Date.now() - 4 * 60 * 60 * 1000).toISOString();
    await renderUi(<LastUsed at={at} />);
    const used = screen.getByText(/^Used .+ ago$/);
    act(() => used.focus());
    expect(await screen.findByText(formatDate(at, { time: true }), {}, { timeout: 3000 })).toBeInTheDocument();
  });
  it("says a key was never used", async () => {
    await renderUi(<LastUsed at={null} />);
    expect(screen.getByText("Never used")).toBeInTheDocument();
  });
});
