import { ThemeImage } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "ThemeImage",
  description: "An image drawn twice, once for each theme, such as a diagram: it follows the theme setting, not only the system's, and prints the light one.",
  group: "Components",
  entry: "theme-image",
  import: "ThemeImage",
  a11y: [
    "Both images carry the same text alternative; only the one for the current theme is shown, so a screen reader reads it once.",
    "Pass aria-describedby with the id of a longer description when the image is a diagram.",
  ],
};
export const examples = [{ id: "diagram", title: "A diagram in both themes", fn: "Diagram" }];
export function Diagram() {
  return <ThemeImage light="/demos/route.light.svg" dark="/demos/route.dark.svg" alt="Route of a request: app, Kaeso, provider" width={364} height={49} />;
}
export function States() {
  return (
    <StateSection title="Widths">
      <div className="flex flex-col gap-24">
        <ThemeImage light="/demos/route.light.svg" dark="/demos/route.dark.svg" alt="Route of a request, at its own width" width={364} height={49} />
        <ThemeImage
          light="/demos/route.light.svg"
          dark="/demos/route.dark.svg"
          alt="Route of a request, shrunk to a narrow column"
          width={364}
          height={49}
          className="max-w-320"
        />
      </div>
    </StateSection>
  );
}
