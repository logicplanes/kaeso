import { useState } from "react";
import { Attachments, type Attachment } from "@logicplanes/kaeso-ui/playground";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Attachments",
  description: "The files sent with a prompt: images as thumbnails, other files by name, each with a remove button while it waits.",
  group: "Playground",
  entry: "playground",
  import: "Attachments",
  barrel: false,
  a11y: [
    "The row is a list named “Attachments”, so a screen reader says how many files wait.",
    "Every remove button is named after its file (“Remove invoice-2026-10.pdf”), never just “Remove”.",
    "An image's name is its text alternative and shows in its place when it can't load.",
  ],
};
export const examples = [
  { id: "waiting", title: "Waiting in the composer", fn: "Waiting" },
  { id: "sent", title: "Above a sent prompt", fn: "Sent" },
];

const files: Attachment[] = [
  { id: "receipt", name: "receipt-northwind.svg", type: "image/svg+xml", src: "/demos/receipt.svg" },
  { id: "screen", name: "settings-screen.svg", type: "image/svg+xml", src: "/demos/screenshot.svg" },
  { id: "invoice", name: "invoice-2026-10.pdf", type: "application/pdf" },
  { id: "standup", name: "standup-notes.m4a", type: "audio/mp4" },
];

export function Waiting() {
  const [items, setItems] = useState(files);
  return <Attachments items={items} onRemove={(id) => setItems((now) => now.filter((a) => a.id !== id))} />;
}
export function Sent() {
  return (
    <div className="flex w-full max-w-480 flex-col">
      <Attachments items={files.slice(0, 3)} align="end" />
    </div>
  );
}
export function States() {
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="Images and files, removable">
        <Attachments items={files} onRemove={() => undefined} />
      </StateSection>
      <StateSection title="Sent, at the end of the line">
        <Attachments items={files} align="end" />
      </StateSection>
      <StateSection title="A long file name">
        <Attachments
          items={[{ id: "long", name: "quarterly-provider-spend-by-project-2026-q3-final-reviewed.pdf", type: "application/pdf" }]}
          onRemove={() => undefined}
        />
      </StateSection>
      <StateSection title="An image that can't load">
        <Attachments
          items={[{ id: "broken", name: "receipt-northwind.png", type: "image/png", src: "data:image/png;base64,AAAA" }]}
          onRemove={() => undefined}
        />
      </StateSection>
    </div>
  );
}
