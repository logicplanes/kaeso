import { describe, expect, it } from "vitest";
import { MAX_SECONDS, concatList, gifFilter, seconds } from "./demo-frames.mjs";

describe("the README demo's frames", () => {
  it("shows each frame until the next one arrived and holds the last", () => {
    const frames = [
      { file: "00000.jpg", time: 10 },
      { file: "00001.jpg", time: 10.05 },
      { file: "00002.jpg", time: 12.5 },
    ];
    expect(concatList(frames, 0.05).split("\n")).toEqual([
      "ffconcat version 1.0",
      "file '00000.jpg'",
      "duration 0.050",
      "file '00001.jpg'",
      "duration 2.450",
      "file '00002.jpg'",
      "duration 0.050",
      "file '00002.jpg'",
      "",
    ]);
  });

  it("never gives a frame a negative duration when timestamps arrive out of order", () => {
    const list = concatList(
      [
        { file: "a.jpg", time: 3 },
        { file: "b.jpg", time: 2.9 },
      ],
      1,
    );
    expect(list).toContain("file 'a.jpg'\nduration 0.000\n");
  });

  it("plays from the first frame to the end of the last frame's hold, under 15 seconds", () => {
    expect(
      seconds(
        [
          { file: "a.jpg", time: 100 },
          { file: "b.jpg", time: 112.5 },
        ],
        0.05,
      ),
    ).toBeCloseTo(12.55);
    expect(seconds([], 1)).toBe(0);
    expect(MAX_SECONDS).toBe(15);
  });

  it("refuses a recording without frames", () => {
    expect(() => concatList([], 1)).toThrow("demo: the screencast sent no frames");
  });

  it("scales to the GIF's width with one palette for the whole recording", () => {
    expect(gifFilter({ width: 1280, fps: 20 })).toBe(
      "fps=20,scale=1280:-2:flags=lanczos,split[frames][copy];[copy]palettegen=max_colors=256:stats_mode=full[palette];[frames][palette]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
    );
  });
});
