import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { afterEach, describe, expect, it } from "vitest";
import { contextLabel } from "./model-fields";

const limits = [8192, 128_000, 200_000, 1_048_576, 2_000_000];

describe("contextLabel", () => {
  afterEach(() => setFormatLocale("en"));

  it("shows a context window in English short form", () => {
    expect(limits.map(contextLabel)).toEqual(["8.19K", "128K", "200K", "1.05M", "2M"]);
  });

  it("follows German number words", () => {
    setFormatLocale("de");
    expect(limits.map(contextLabel)).toEqual(["8192", "128.000", "200.000", "1,05 Mio.", "2 Mio."]);
  });

  it("counts in ten-thousands in Chinese", () => {
    setFormatLocale("zh-CN");
    expect(limits.map(contextLabel)).toEqual(["8192", "12.8万", "20万", "104.9万", "200万"]);
  });

  it("shows a dash when the limit is unknown", () => {
    expect(contextLabel(null)).toBe("—");
    expect(contextLabel(undefined)).toBe("—");
    expect(contextLabel(0)).toBe("—");
  });
});
