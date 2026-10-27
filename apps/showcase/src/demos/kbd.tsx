import { Button, Kbd, KbdSequence } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Kbd",
  description: "Key caps and shortcuts: ⌘ ⌥ ⇧ ⌃ on macOS, words elsewhere; sequences like G then M.",
  group: "Components",
  entry: "kbd",
  import: "Kbd, KbdSequence",
  a11y: [
    "A symbol such as ⌘ is hidden from screen readers and its name (“Command”) is read instead; letters need no extra name.",
    "Inside a button, the shortcut stays out of the button's name, which is its visible label.",
  ],
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
      <StateSection title="Long key names">
        <div className="flex flex-wrap items-center gap-8">
          <Kbd>backspace</Kbd>
          <Kbd>tab</Kbd>
          <Kbd>Rücktaste</Kbd>
          <Kbd>PgDn</Kbd>
        </div>
      </StateSection>
      <StateSection title="Sequences and ink surfaces">
        <div className="flex flex-wrap items-center gap-24">
          <Sequence />
          <KbdSequence keys={[["g"], ["g"]]} />
          <span className="inline-flex h-44 items-center gap-8 rounded-pill bg-ink px-16 text-body font-medium text-on-ink">
            Undo <KbdSequence keys={["mod", "z"]} tone="onInk" />
          </span>
        </div>
      </StateSection>
      <StateSection title="In buttons">
        <div className="flex flex-wrap items-center gap-12">
          <Button size="lg" shortcut={["mod", "enter"]}>
            Send
          </Button>
          <Button size="lg" variant="primary" shortcut={["mod", "enter"]}>
            Send
          </Button>
          <Button size="lg" variant="danger" shortcut={["mod", "backspace"]}>
            Delete key
          </Button>
        </div>
      </StateSection>
    </>
  );
}
