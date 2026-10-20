import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { afterEach, describe, expect, it } from "vitest";
import { contextLabel } from "./model-fields";

const limits = [8192, 128_000, 200_000, 1_048_576, 2_000_000];

describe("contextLabel", () => {
  afterEach(() => setFormatLocale("en"));

  it("names a context window as models are named", () => {
    expect(limits.map(contextLabel)).toEqual(["8K", "128K", "200K", "1M", "2M"]);
    expect(contextLabel(131_072)).toBe("128K");
  });

  it("keeps the same short form in German and Chinese", () => {
    setFormatLocale("de");
    expect(limits.map(contextLabel)).toEqual(["8K", "128K", "200K", "1M", "2M"]);
    expect(contextLabel(1_536)).toBe("1,5K");
    setFormatLocale("zh-CN");
    expect(limits.map(contextLabel)).toEqual(["8K", "128K", "200K", "1M", "2M"]);
  });

  it("shows a dash when the limit is unknown", () => {
    expect(contextLabel(null)).toBe("—");
    expect(contextLabel(undefined)).toBe("—");
    expect(contextLabel(0)).toBe("—");
  });
});
