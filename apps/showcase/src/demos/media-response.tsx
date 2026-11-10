import { useEffect, useState } from "react";
import { MediaResponse, ResponseMetrics, type MediaStatus } from "@logicplanes/kaeso-ui/playground";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "MediaResponse",
  description: "What an image, speech, sound or video model made, in a response's place: images in a grid, audio and video in players, each with Download.",
  group: "Playground",
  entry: "playground",
  import: "MediaResponse",
  barrel: false,
  a11y: [
    "The response is an article named after its model and busy while it is made; screen readers hear when it finishes.",
    "Each Download is a link named after its file, so a list of links reads “Download image-1.png”, not “Download”.",
    "A video job's progress is a progressbar whose name says how far it is, and Cancel is a real button beside it.",
    "Placeholders pulse only without reduced motion; with it, they hold still.",
  ],
};
export const examples = [
  { id: "images", title: "Images", fn: "Images", minHeight: 360 },
  { id: "video", title: "A video job", description: "The job's progress and Cancel while it runs, then the video.", fn: "Video", minHeight: 360 },
];

const receipt = { src: "/demos/receipt.svg", name: "image-1.svg", alt: "A receipt from Northwind Labs" };
const screen = { src: "/demos/screenshot.svg", name: "image-2.svg", alt: "A settings screen" };
const metrics = <ResponseMetrics duration={8240} cost="0.0842" />;

export function Images() {
  return (
    <div className="w-full max-w-480">
      <MediaResponse kind="image" items={[receipt, screen]} footer={metrics} onRetry={() => undefined} aria-label="GPT Image 2 response" />
    </div>
  );
}
export function Video() {
  const [progress, setProgress] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setProgress((p) => (p >= 100 ? 0 : p + 10)), 600);
    return () => clearInterval(id);
  }, []);
  const status: MediaStatus = progress === 0 ? "pending" : "running";
  return (
    <div className="w-full max-w-480">
      <MediaResponse kind="video" status={status} progress={progress} onCancel={() => setProgress(0)} aria-label="Grok Imagine Video response" />
    </div>
  );
}
export function States() {
  const none = () => undefined;
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="Two images on their way">
        <MediaResponse kind="image" status="pending" expected={2} footer={<ResponseMetrics />} />
      </StateSection>
      <StateSection title="Two images">
        <MediaResponse kind="image" items={[receipt, screen]} footer={metrics} onRetry={none} />
      </StateSection>
      <StateSection title="One image">
        <MediaResponse kind="image" items={[receipt]} footer={<ResponseMetrics duration={6100} costKnown={false} />} />
      </StateSection>
      <StateSection title="Audio on its way">
        <MediaResponse kind="audio" status="running" />
      </StateSection>
      <StateSection title="Audio">
        <MediaResponse
          kind="audio"
          items={[{ src: "data:audio/mpeg;base64,", name: "speech.mp3" }]}
          footer={<ResponseMetrics duration={1320} cost="0.000225" />}
        />
      </StateSection>
      <StateSection title="A video job waiting to start">
        <MediaResponse kind="video" status="pending" onCancel={none} />
      </StateSection>
      <StateSection title="A video job, 40% done">
        <MediaResponse kind="video" status="running" progress={40} onCancel={none} />
      </StateSection>
      <StateSection title="Stopped">
        <MediaResponse kind="video" status="stopped" onRetry={none} />
      </StateSection>
      <StateSection title="Failed">
        <MediaResponse kind="image" status="error" error="The provider refused the request as invalid; check the model's settings." onRetry={none} />
      </StateSection>
      <StateSection title="Nothing to show">
        <MediaResponse kind="audio" items={[]} />
      </StateSection>
    </div>
  );
}
