import { Skeleton, SkeletonLine } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Skeleton",
  description: "Shapes of the content that is loading, with a quiet pulse, never a shimmer.",
  group: "Components",
  entry: "skeleton",
  import: "Skeleton, SkeletonLine",
  a11y: ["Skeletons are hidden from assistive technology; mark the region aria-busy while it loads."],
};
export const examples = [
  { id: "row", title: "Rows", fn: "Rows" },
  { id: "metric", title: "Metric row", fn: "Metric" },
];
export function Rows() {
  return (
    <div className="flex w-480 flex-col gap-4">
      {[64, 48, 72].map((w, i) => (
        <div key={i} className="flex h-44 items-center gap-12 px-16">
          <Skeleton shape="circle" className="size-16" />
          <Skeleton shape="line" style={{ width: `${w}%` }} />
        </div>
      ))}
    </div>
  );
}
export function Metric() {
  return (
    <div className="flex flex-wrap gap-x-48 gap-y-24">
      {[0, 1, 2].map((i) => (
        <div key={i} className="flex flex-col gap-8">
          <Skeleton className="h-32 w-120" />
          <SkeletonLine className="w-80" />
        </div>
      ))}
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Shapes">
        <div className="flex items-center gap-16">
          <Skeleton shape="circle" className="size-32" />
          <Skeleton shape="control" className="w-120" />
          <Skeleton className="h-80 w-160" />
          <SkeletonLine lines={3} className="w-240" />
        </div>
      </StateSection>
      <StateSection title="Rows">
        <Rows />
      </StateSection>
    </>
  );
}
