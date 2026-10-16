import { Kbd, KbdSequence } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Kbd",
  description: "Key caps and shortcuts: ⌘ ⌥ ⇧ ⌃ on macOS, words elsewhere; sequences like G then M.",
  group: "Components",
  entry: "kbd",
  import: "Kbd, KbdSequence",
  a11y: ["Modifier symbols carry an aria-label with the key's name."],
};
export const examples = [
  { id: "keys", title: "Keys", fn: "Keys" },
  { id: "sequence", title: "Sequences", fn: "Sequence" },
];
export function Keys() {
  return (
    <div className="flex items-center gap-8">
      <Kbd>mod</Kbd>
      <Kbd>shift</Kbd>
      <Kbd>alt</Kbd>
      <Kbd>enter</Kbd>
      <Kbd>esc</Kbd>
      <Kbd>k</Kbd>
    </div>
  );
}
export function Sequence() {
  return (
    <div className="flex flex-col items-start gap-12">
      <KbdSequence keys={["mod", "k"]} />
      <KbdSequence keys={[["g"], ["l"]]} />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Keys">
        <Keys />
      </StateSection>
      <StateSection title="Sequences and ink surfaces">
        <div className="flex items-center gap-24">
          <Sequence />
          <span className="inline-flex h-44 items-center gap-8 rounded-pill bg-ink px-16 text-body font-medium text-on-ink">
            Undo <KbdSequence keys={["mod", "z"]} tone="onInk" />
          </span>
        </div>
      </StateSection>
    </>
  );
}
