import { describe, expect, it } from "vitest";
import { mediaShort } from "./media";

describe("a media request's short form", () => {
  it("names the audio it made or was sent, and a dash for none", () => {
    expect(mediaShort({ unit: "millisecond", input: 0, output: 2500 })).toBe("2.5 s");
    expect(mediaShort({ unit: "millisecond", input: 4000, output: 0 })).toBe("4 s");
    expect(mediaShort({ unit: "millisecond", input: 0, output: 0 })).toBe("—");
  });

  it("counts videos with their length once one is made", () => {
    expect(mediaShort({ unit: "video", input: 1, output: 0 })).toBe("1 video");
    expect(mediaShort({ unit: "video", input: 2, output: 16000 })).toMatch(/^2 videos · 16/);
  });
});
