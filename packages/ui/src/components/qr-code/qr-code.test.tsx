import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { QRCode } from "./qr-code.js";

describe("QRCode", () => {
  it("draws the value's modules as one path inside a labelled image", () => {
    render(<QRCode value="otpauth://totp/Kaeso:owner%40example.com?secret=JBSWY3DPEHPK3PXP" label="QR code to add Kaeso to an authenticator app" />);
    const svg = screen.getByRole("img", { name: "QR code to add Kaeso to an authenticator app" });
    const [, , w, h] = (svg.getAttribute("viewBox") ?? "").split(" ").map(Number);
    expect(w).toBe(h);
    expect((w ?? 0) % 4).toBe(1);
    const d = svg.querySelector("path")?.getAttribute("d") ?? "";
    expect(d).toMatch(/^M\d+ \d+h1v1h-1z/);
    const xs = [...d.matchAll(/M(\d+) (\d+)/g)].flatMap((m) => [Number(m[1]), Number(m[2])]);
    expect(Math.min(...xs)).toBe(4);
    expect(Math.max(...xs)).toBe((w ?? 0) - 5);
  });

  it("keeps its square size when text beside it in a row needs the room", () => {
    render(
      <div style={{ display: "flex", width: 200 }}>
        <QRCode value="https://accounts.example.com/device?user_code=ABCD-EFGH" label="QR code to open the sign-in page" />
        <p>Open the page on any device and enter the code.</p>
      </div>,
    );
    expect(screen.getByRole("img", { name: "QR code to open the sign-in page" })).toHaveClass("shrink-0", "size-160");
  });
});
