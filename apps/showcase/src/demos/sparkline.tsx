import { Sparkline } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Sparkline",
  description: "One series inline: 80×24, the last point as a dot.",
  group: "Charts",
  entry: "charts",
  import: "Sparkline",
};
export const examples = [
  { id: "inline", title: "Inline", fn: "InlineDemo" },
  { id: "fluid", title: "Fluid", fn: "Fluid", description: "fluid fills its container's width, as in a StatTile; the line keeps its weight at any width." },
];
export function Fluid() {
  return (
    <div className="flex w-full max-w-480 flex-col gap-16">
      <Sparkline fluid aria-label="Requests over 14 days, rising" values={[12, 18, 15, 22, 28, 26, 34, 31, 36, 40, 38, 44, 47, 52]} />
      <div className="w-200">
        <Sparkline fluid aria-label="Errors over 14 days, falling" color="var(--k-viz-4)" values={[9, 8, 8, 7, 6, 7, 5, 5, 4, 4, 3, 3, 2, 2]} />
      </div>
    </div>
  );
}
export function InlineDemo() {
  return (
    <div className="flex items-center gap-12 text-body text-fg">
      <span className="mono text-small">gpt-5</span>
      <Sparkline aria-label="Requests over 7 days, rising" values={[12, 18, 15, 22, 28, 26, 34]} />
      <span className="tabular">34</span>
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Colors">
      <div className="flex flex-col gap-12">
        <Sparkline aria-label="Blue" values={[3, 5, 4, 8, 7, 9, 12]} />
        <Sparkline aria-label="Emerald" color="var(--k-viz-3)" values={[9, 7, 8, 6, 7, 5, 4]} />
        <Sparkline aria-label="Flat" values={[5, 5, 5, 5, 5]} />
        <div className="w-320">
          <Sparkline fluid aria-label="Fluid" values={[3, 5, 4, 8, 7, 9, 12]} />
        </div>
      </div>
    </StateSection>
  );
}
