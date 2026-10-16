import { IconButton, Inline, icons, KAESO_ICON_BUTTON_VARIANTS, type ButtonSize } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "IconButton",
  description: "A circle with an icon, a required name, and a tooltip that says it.",
  group: "Components",
  entry: "icon-button",
  import: "IconButton",
  keyboard: [
    ["Enter", "Activates"],
    ["Space", "Activates"],
  ],
  a11y: [
    "aria-label is required by the types; it is also the tooltip, so sighted and screen-reader users get the same name.",
    "Shortcuts appear in the tooltip as key caps.",
    "Disabled icon buttons have no tooltip unless they carry a disabledReason.",
  ],
};

export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "variants", title: "Variants", fn: "Variants", description: "Ghost in rows and toolbars; secondary for the raised circles in page headers." },
  { id: "sizes", title: "Sizes", fn: "Sizes" },
  { id: "shortcut", title: "With a shortcut", fn: "WithShortcut" },
];

export function Basic() {
  return <IconButton icon={icons.filter} aria-label="Filter" />;
}
export function Variants() {
  return (
    <Inline gap={12}>
      <IconButton icon={icons.filter} aria-label="Filter" variant="secondary" />
      <IconButton icon={icons.displayOptions} aria-label="Display options" variant="secondary" />
      <IconButton icon={icons.sidePanel} aria-label="Toggle side panel" variant="secondary" />
      <IconButton icon={icons.more} aria-label="More actions" />
      <IconButton icon={icons.add} aria-label="Create key" variant="primary" />
    </Inline>
  );
}
export function Sizes() {
  return (
    <Inline gap={12}>
      {(["xs", "sm", "md", "lg", "xl"] as ButtonSize[]).map((s) => (
        <IconButton key={s} size={s} icon={icons.add} aria-label="Add" variant="secondary" />
      ))}
    </Inline>
  );
}
export function WithShortcut() {
  return <IconButton icon={icons.search} aria-label="Search" shortcut={["mod", "k"]} variant="secondary" />;
}
export function States() {
  return (
    <>
      <Matrix
        title="Variants × states"
        rows={Object.keys(KAESO_ICON_BUTTON_VARIANTS.variant).map((v) => ({ label: v }))}
        render={(r, p) => <IconButton icon={icons.filter} aria-label="Filter" variant={r.label as "ghost"} tooltip={false} {...p} />}
      />
      <StateSection title="Sizes">
        <Inline gap={12}>
          {(["xs", "sm", "md", "lg", "xl"] as ButtonSize[]).map((s) => (
            <IconButton key={s} size={s} icon={icons.more} aria-label="More" variant="secondary" tooltip={false} />
          ))}
        </Inline>
      </StateSection>
    </>
  );
}
