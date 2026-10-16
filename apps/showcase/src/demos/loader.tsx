import { useState } from "react";
import { Button, Inline, Loader, Spinner, icons } from "@logicplanes/kaeso-ui";
import { StateMatrix, StateRow, StateCell, StateSection } from "~/lib/states";

export const meta = {
  title: "Loader",
  description:
    "Kaeso's spinner: three routes on a ring, the lead in full color, advancing a third of a turn per 800ms beat. In controls it shows only once work has taken 300ms, then for at least 400ms.",
  group: "Components",
  entry: "loader",
  import: "Loader, Spinner, useDelayedLoading",
  a11y: [
    "Loader is a status region: its label, or “Loading”, is announced when it shows; while it waits out its delay it keeps its place and is hidden from assistive technology.",
    "A Spinner inside a busy control stays silent: the control says it is busy (aria-busy) at once.",
    "With reduced motion the ring holds still and the light passes from route to route by opacity alone; nothing turns or flashes.",
  ],
};
export const examples = [
  { id: "inline", title: "Inline", fn: "InlineLoader", description: "A status with a label, in secondary text." },
  {
    id: "sizes",
    title: "Sizes",
    fn: "Sizes",
    description:
      "14 and 16px in controls and toasts, 20px in xl buttons, 24px for a region, 32px for a page. The stroke is 1.5px at every size, like an icon's at 24px.",
  },
  {
    id: "delayed",
    title: "Delay and minimum",
    fn: "Delayed",
    description:
      "Quick work shows nothing; slower work shows the spinner after 300ms and keeps it at least 400ms, so it never flashes or flickers. Buttons do this by themselves.",
  },
  { id: "page", title: "A page loading", fn: "PageLoading", description: "The quietest tone at 32px, after the delay." },
];
export function InlineLoader() {
  return <Loader label="Saving provider…" />;
}
export function Sizes() {
  return (
    <div className="flex items-center gap-16 text-fg">
      <Spinner size="xs" />
      <Spinner size="sm" />
      <Spinner size="md" />
      <Spinner size="lg" />
      <Spinner size="xl" />
    </div>
  );
}
export function Delayed() {
  const [quick, setQuick] = useState(false);
  const [slow, setSlow] = useState(false);
  return (
    <Inline gap={12}>
      <Button
        loading={quick}
        onClick={() => {
          setQuick(true);
          setTimeout(() => setQuick(false), 150);
        }}
      >
        Quick save (150ms)
      </Button>
      <Button
        variant="primary"
        loading={slow}
        onClick={() => {
          setSlow(true);
          setTimeout(() => setSlow(false), 1800);
        }}
      >
        Slow save (1.8s)
      </Button>
    </Inline>
  );
}
export function PageLoading() {
  return (
    <div className="flex h-160 w-full items-center justify-center rounded-well bg-fill-1">
      <Loader size="xl" tone="tertiary" delay />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateMatrix title="Sizes and tones" columns={["xs · 14", "sm · 16", "md · 20", "lg · 24", "xl · 32"]}>
        {(["current", "secondary", "tertiary"] as const).map((tone) => (
          <StateRow key={tone} label={tone}>
            {(["xs", "sm", "md", "lg", "xl"] as const).map((size) => (
              <StateCell key={size} id={`${tone}-${size}`}>
                <span className={tone === "current" ? "flex text-fg" : tone === "secondary" ? "flex text-fg-secondary" : "flex text-fg-tertiary"}>
                  <Spinner size={size} />
                </span>
              </StateCell>
            ))}
          </StateRow>
        ))}
      </StateMatrix>
      <StateSection title="In controls (busy, after the delay)">
        <Inline gap={12}>
          <Button variant="primary" loading>
            Save changes
          </Button>
          <Button loading leadingIcon={icons.refresh}>
            Syncing models
          </Button>
          <Button variant="danger" loading>
            Delete key
          </Button>
          <Button variant="ghost" size="xs" loading leadingIcon={icons.refresh}>
            Retry
          </Button>
          <Button variant="primary" size="xl" loading>
            Continue
          </Button>
        </Inline>
      </StateSection>
      <StateSection title="Labels">
        <div className="flex flex-col gap-12">
          <Loader label="Saving provider…" />
          <Loader size="xs" label="Loading results" className="text-small" />
        </div>
      </StateSection>
      <StateSection title="On fills">
        <Inline gap={12}>
          {["bg-fill-1", "bg-fill-2", "bg-raised shadow-xs", "material-primary"].map((c) => (
            <span key={c} className={`flex size-48 items-center justify-center rounded-well ${c}`}>
              <Spinner size="lg" />
            </span>
          ))}
        </Inline>
      </StateSection>
    </>
  );
}
