import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Link } from "@logicplanes/kaeso-ui";
import { renderUi } from "~/test/render";

describe("links inside the console", () => {
  it("keep their target when they open in a new tab", async () => {
    await renderUi(
      <Link href="/settings/account/profile" external>
        Profile
      </Link>,
    );
    const link = screen.getByRole("link", { name: /Profile/ });
    expect(link).toHaveAttribute("href", "/settings/account/profile");
    expect(link).toHaveAttribute("target", "_blank");
  });
});
