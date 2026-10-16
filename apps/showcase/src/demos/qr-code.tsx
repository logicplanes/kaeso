import { QRCode } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

const uri = "otpauth://totp/Kaeso:owner%40example.com?secret=JBSWY3DPEHPK3PXP&issuer=Kaeso";

export const meta = {
  title: "QRCode",
  description: "A scannable code drawn as one SVG path: dark modules on white in both themes, with a quiet zone.",
  group: "Components",
  entry: "qr-code",
  import: "QRCode",
  a11y: ["The code is an image with a label that says what it is for; always offer the same value as text next to it."],
};
export const examples = [{ id: "authenticator", title: "Authenticator setup", fn: "Authenticator" }];
export function Authenticator() {
  return <QRCode value={uri} label="QR code to add Kaeso to an authenticator app" />;
}
export function States() {
  return (
    <StateSection title="Sizes">
      <div className="flex items-end gap-24">
        <QRCode value={uri} label="QR code, medium" />
        <QRCode value={uri} label="QR code, large" size="lg" />
      </div>
    </StateSection>
  );
}
