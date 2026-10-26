import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Icon } from "./icon.js";
import { icons } from "../../icons/vocabulary.js";

describe("Icon", () => {
  it("is hidden by default and keeps its name when labelled, whatever else it is given", () => {
    const { container } = render(
      <>
        <Icon icon={icons.key} />
        <Icon icon={icons.key} label="Key" aria-hidden />
      </>,
    );
    expect(container.querySelectorAll("svg")[0]).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByRole("img", { name: "Key" })).not.toHaveAttribute("aria-hidden");
  });
});
