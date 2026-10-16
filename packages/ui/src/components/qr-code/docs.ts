import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  QRCode: {
    description: "A QR code drawn as one SVG path: dark modules on white in both themes, with a quiet zone, so cameras read it whatever the console's theme.",
    props: {
      label: 'What it is for, read by screen readers: "QR code to add Kaeso to an authenticator app".',
      size: "The code's size.",
      value: "What the code holds, e.g. an `otpauth://` URI.",
    },
  },
} satisfies ComponentDocs;
