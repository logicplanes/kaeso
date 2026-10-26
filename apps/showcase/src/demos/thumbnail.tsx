import { Thumbnail } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Thumbnail",
  description: "A square image someone sent, such as a photo in a request's conversation, cropped to fill a well.",
  group: "Components",
  entry: "thumbnail",
  import: "Thumbnail",
  a11y: [
    "Alt is required: say what the image shows, or the sender's own description when the request carried one.",
    "When the image can't load, its text alternative takes its place, so nothing is lost.",
  ],
};
export const examples = [
  { id: "sent", title: "An image in a message", fn: "Sent" },
  { id: "sizes", title: "Sizes", fn: "Sizes" },
];
export function Sent() {
  return <Thumbnail src="/demos/receipt.svg" alt="A receipt from Northwind Labs" />;
}
export function Sizes() {
  return (
    <div className="flex flex-wrap items-end gap-12">
      <Thumbnail src="/demos/receipt.svg" alt="A receipt, 64px" size="sm" />
      <Thumbnail src="/demos/receipt.svg" alt="A receipt, 120px" />
      <Thumbnail src="/demos/receipt.svg" alt="A receipt, 160px" size="lg" />
    </div>
  );
}
export function States() {
  return (
    <div>
      <StateSection title="Sizes">
        <Sizes />
      </StateSection>
      <StateSection title="A white image on the panel">
        <div className="flex flex-wrap items-end gap-12">
          <Thumbnail src="/demos/screenshot.svg" alt="A white settings screen" size="sm" />
          <Thumbnail src="/demos/screenshot.svg" alt="A white settings screen" />
        </div>
      </StateSection>
      <StateSection title="Can't load">
        <div className="flex flex-wrap items-end gap-12">
          <Thumbnail src="data:image/png;base64,AAAA" alt="A receipt from Northwind Labs" size="sm" />
          <Thumbnail src="data:image/png;base64,AAAA" alt="A receipt from Northwind Labs" />
          <Thumbnail src="data:image/png;base64,AAAA" alt="A receipt from Northwind Labs" size="lg" />
        </div>
      </StateSection>
    </div>
  );
}
