import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { ProviderBadge } from "./identity.js";
import { PROVIDER_MARKS } from "./provider-marks.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

vi.mock("./vendor-marks.js", () => {
  throw new Error("Failed to fetch dynamically imported module: http://example.com/vendor-marks.js");
});

describe("a model's maker mark that can't load", () => {
  it("shows the provider's own mark and name instead of an empty tile", async () => {
    render(<ProviderBadge provider="amazon-bedrock" name="Amazon Bedrock" model="us.meta.llama4-maverick-17b-instruct-v1:0" />);
    expect(screen.getByRole("img", { name: "Meta model on Amazon Bedrock" })).toHaveTextContent(/^$/);
    await act(() => lazyModulesSettled());
    const tile = screen.getByRole("img", { name: "Amazon Bedrock" });
    expect(tile.querySelector("path")).toHaveAttribute("d", PROVIDER_MARKS["amazon-bedrock"]!.d);
  });
});
