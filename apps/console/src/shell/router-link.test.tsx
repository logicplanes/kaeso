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

  it("go where the browser would resolve them", async () => {
    await renderUi(<Link href="/settings/../orgs/kaeso?tab=keys">Keys</Link>);
    expect(screen.getByRole("link", { name: "Keys" })).toHaveAttribute("href", "/orgs/kaeso?tab=keys");
  });

  it("keep addresses on the web, mail and authenticator apps as they are", async () => {
    await renderUi(
      <>
        <Link href="https://docs.kaeso.app/self-hosting/upgrades">Docs</Link>
        <Link href="mailto:owner@example.com">Mail</Link>
        <Link href="otpauth://totp/Kaeso:owner@example.com?secret=JBSWY3DPEHPK3PXP">App</Link>
      </>,
    );
    expect(screen.getByRole("link", { name: /^Docs/ })).toHaveAttribute("href", "https://docs.kaeso.app/self-hosting/upgrades");
    expect(screen.getByRole("link", { name: /^Mail/ })).toHaveAttribute("href", "mailto:owner@example.com");
    expect(screen.getByRole("link", { name: "App" })).toHaveAttribute("href", "otpauth://totp/Kaeso:owner@example.com?secret=JBSWY3DPEHPK3PXP");
  });

  it("never lead to another origin or run a script", async () => {
    const hrefs = [
      "//evil.example",
      "/.//evil.example",
      "/%2e//evil.example",
      "/a/..//evil.example",
      "/./\\evil.example",
      "javascript://%0Aalert(1)",
      "data:text/html,x",
    ];
    await renderUi(
      <>
        {hrefs.map((href) => (
          <Link key={href} href={href}>
            {href}
          </Link>
        ))}
      </>,
    );
    for (const href of hrefs) {
      const text = screen.getByText(href);
      expect(text.closest("a"), href).not.toHaveAttribute("href");
    }
  });
});
