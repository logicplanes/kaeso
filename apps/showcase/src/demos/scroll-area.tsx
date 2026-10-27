import { useEffect, useRef } from "react";
import { ScrollArea } from "@logicplanes/kaeso-ui";
import { models } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "ScrollArea",
  description: "A scroll container with thin scrollbars that appear on hover, fading at every edge with more to scroll to.",
  group: "Components",
  entry: "scroll-area",
  import: "ScrollArea",
  a11y: [
    "Give a focusable scroll region an aria-label so keyboard users know what they scroll.",
    "The focus ring is drawn above the fade, so all four sides show while the content scrolls.",
  ],
};
export const examples = [{ id: "basic", title: "Basic", fn: "Basic" }];
export function Basic() {
  return (
    <ScrollArea className="h-160 w-320 rounded-well bg-fill-1" aria-label="Models">
      <ul className="flex flex-col p-12">
        {[...models, ...models, ...models].map((m, i) => (
          <li key={i} className="flex h-32 items-center mono text-small text-fg">
            {m.model}
          </li>
        ))}
      </ul>
    </ScrollArea>
  );
}
function Scrolled({ orientation, focus = false }: { orientation: "vertical" | "horizontal" | "both"; focus?: boolean }) {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const viewport = root.current?.querySelector<HTMLElement>("[role=region]");
    if (!viewport) return;
    if (orientation !== "horizontal") viewport.scrollTop = 64;
    if (orientation !== "vertical") viewport.scrollLeft = 120;
    if (focus) viewport.setAttribute("data-force", "focus");
  }, [focus, orientation]);
  const wide = orientation !== "vertical";
  return (
    <ScrollArea ref={root} orientation={orientation} className="h-160 w-280 rounded-well bg-fill-1" aria-label={`Models, ${orientation}`}>
      <ul className={wide ? "flex w-560 flex-col p-12" : "flex flex-col p-12"}>
        {[...models, ...models, ...models].map((m, i) => (
          <li key={i} className="flex h-32 items-center mono text-small whitespace-nowrap text-fg">
            {wide ? `${m.model} · ${m.provider} · 128k context · $0.25 per million input tokens` : m.model}
          </li>
        ))}
      </ul>
    </ScrollArea>
  );
}

export function States() {
  return (
    <>
      <StateSection title="Vertical: at the top, and scrolled with keyboard focus">
        <div className="flex gap-24">
          <Basic />
          <Scrolled orientation="vertical" focus />
        </div>
      </StateSection>
      <StateSection title="Sideways and both ways, scrolled to the middle">
        <div className="flex gap-24">
          <Scrolled orientation="horizontal" />
          <Scrolled orientation="both" />
        </div>
      </StateSection>
    </>
  );
}
