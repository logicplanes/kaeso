import { Figure, ThemeImage } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Figure",
  description: "A diagram or image set apart from the text: centred on a well, with a caption, scrolling sideways only when it can't shrink to fit.",
  group: "Components",
  entry: "figure",
  import: "Figure",
  a11y: [
    "The caption names the figure; the label names the region that scrolls, so keyboard users know what they scroll.",
    "Put a diagram's longer description in a visually hidden element and point the image's aria-describedby at it.",
  ],
};
export const examples = [{ id: "diagram", title: "A diagram with its caption", fn: "Diagram" }];
export function Diagram() {
  return (
    <Figure label="Diagram: route of a request" caption="Route of a request" className="w-full">
      <ThemeImage light="/demos/route.light.svg" dark="/demos/route.dark.svg" alt="Route of a request: app, Kaeso, provider" width={364} height={49} />
    </Figure>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Without a caption">
        <Figure label="Diagram: route of a request, without a caption" className="w-full">
          <ThemeImage light="/demos/route.light.svg" dark="/demos/route.dark.svg" alt="Route of a request: app, Kaeso, provider" width={364} height={49} />
        </Figure>
      </StateSection>
      <StateSection title="Wider than its column">
        <Figure label="Diagram: route of a request, wider than its column" caption="Route of a request" className="w-280">
          <ThemeImage
            light="/demos/route.light.svg"
            dark="/demos/route.dark.svg"
            alt="Route of a request: app, Kaeso, provider"
            width={364}
            height={49}
            className="min-w-360"
          />
        </Figure>
      </StateSection>
    </>
  );
}
