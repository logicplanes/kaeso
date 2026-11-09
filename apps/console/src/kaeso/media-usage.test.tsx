import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { usageRow } from "~/test/fixtures/project";
import { renderUi } from "~/test/render";
import { hasMedia, mediaSummary, UsageCost } from "./media-usage";

const none = { images: 0, characters: 0, audio_ms: 0, videos: 0, video_ms: 0 };

describe("media usage", () => {
  it("sums what media requests made, and nothing for a group without media", () => {
    expect(mediaSummary(undefined)).toBe("");
    expect(mediaSummary(none)).toBe("");
    expect(mediaSummary({ ...none, images: 1 })).toBe("1 image");
    expect(mediaSummary({ ...none, videos: 3 })).toBe("3 videos");
    expect(mediaSummary({ ...none, images: 2, characters: 1200, videos: 1, video_ms: 8000 })).toMatch(/^2 images · 1,200 characters · 1 video, 8/);
    expect(hasMedia([usageRow(), usageRow({ media: { ...none, audio_ms: 900 } })])).toBe(true);
    expect(hasMedia([usageRow()])).toBe(false);
  });

  it("shows a cost no price covers as unknown, and counts the unknown requests beside a known cost", async () => {
    const { unmount } = await renderUi(<UsageCost row={usageRow({ requests: 2, unpriced: 2, cost: "0" })} />);
    expect(screen.getByText("Unknown")).toBeInTheDocument();
    unmount();
    const partly = await renderUi(<UsageCost row={usageRow({ requests: 3, unpriced: 1, cost: "0.5" })} />);
    expect(screen.getByText("1 request unknown")).toBeInTheDocument();
    partly.unmount();
    const hidden = await renderUi(<UsageCost row={usageRow({ cost: null })} />);
    expect(screen.getByText("—")).toBeInTheDocument();
    hidden.unmount();
    await renderUi(<UsageCost row={usageRow({ cost: "1.25" })} />);
    expect(screen.queryByText(/unknown/i)).not.toBeInTheDocument();
  });
});
