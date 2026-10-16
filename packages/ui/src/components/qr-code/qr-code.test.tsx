import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { QRCode } from "./qr-code";

describe("QRCode", () => {
  it("draws the value's modules as one path inside a labelled image", () => {
    render(<QRCode value="otpauth://totp/Kaeso:owner%40example.com?secret=JBSWY3DPEHPK3PXP" label="QR code to add Kaeso to an authenticator app" />);
    const svg = screen.getByRole("img", { name: "QR code to add Kaeso to an authenticator app" });
    const [, , w, h] = (svg.getAttribute("viewBox") ?? "").split(" ").map(Number);
    expect(w).toBe(h);
    expect((w ?? 0) % 4).toBe(1);
    expect(svg.querySelector("path")?.getAttribute("d")).toMatch(/^M\d+ \d+h1v1h-1z/);
  });
});
