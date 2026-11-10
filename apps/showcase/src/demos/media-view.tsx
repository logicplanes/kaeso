import { MediaView } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

// One second of silence as a 8 kHz mono WAV, so the player has something real to load.
const SILENCE = `data:audio/wav;base64,UklGRkQfAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YSAfAACA${"gICA".repeat(2666)}`;

export const meta = {
  title: "MediaView",
  description: "An image, an audio clip, a video or a file from a request, with a line that names it and a download.",
  group: "Components",
  entry: "thumbnail",
  import: "MediaView",
  a11y: [
    "The label is required: it is the image's text alternative, the player's accessible name and the caption.",
    "An image opens larger from a button named after it, which says whether it is open and closes it again.",
    "Audio and video use the browser's own controls, which work with the keyboard and assistive technology.",
  ],
  dos: ["Pass the format, dimensions and size as the detail, so people know what a download holds."],
  donts: ["Don't pass an address from logged or pasted content without checking its type: an SVG or HTML file is a document, not an image."],
};
export const examples = [
  { id: "image", title: "An image a request carried", fn: "Image" },
  { id: "audio", title: "An audio clip", fn: "Audio" },
  { id: "file", title: "A file to download", fn: "File" },
];
export function Image() {
  return <MediaView src="/demos/receipt.svg" label="Image" detail="PNG, 1,024 × 768, 1.2 MB" filename="image-1.png" />;
}
export function Audio() {
  return <MediaView kind="audio" src={SILENCE} label="Audio" detail="WAV, 7.8 KB" filename="audio-1.wav" />;
}
export function File() {
  return <MediaView kind="file" src="/demos/receipt.svg" label="invoice.pdf" detail="PDF, 90.0 KB" filename="invoice.pdf" />;
}
export function States() {
  return (
    <div>
      <StateSection title="Image">
        <Image />
      </StateSection>
      <StateSection title="Audio">
        <Audio />
      </StateSection>
      <StateSection title="Video">
        <MediaView kind="video" src={SILENCE} label="Video" detail="MP4, 2.0 MB" filename="video-1.mp4" />
      </StateSection>
      <StateSection title="A file to download">
        <File />
      </StateSection>
      <StateSection title="Without a detail">
        <MediaView kind="file" src="/demos/receipt.svg" label="notes.txt" />
      </StateSection>
    </div>
  );
}
