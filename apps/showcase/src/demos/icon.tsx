import { Icon, icons, type IconName } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Icon",
  description: "Hugeicons free, Stroke Rounded, in currentColor. One icon per Kaeso concept.",
  group: "Components",
  entry: "icon",
  import: "Icon, icons",
  a11y: ["Icons are decorative by default (aria-hidden). Pass label when the icon alone carries meaning."],
};
export const examples = [
  { id: "sizes", title: "Sizes", fn: "Sizes", description: "14 (1.75 stroke), 16, 18, 20, 24." },
  { id: "vocabulary", title: "Vocabulary", fn: "Vocabulary" },
];
export function Sizes() {
  return (
    <div className="flex items-end gap-16 text-fg">
      {(["xs", "sm", "md", "lg", "xl"] as const).map((s) => (
        <Icon key={s} icon={icons.key} size={s} />
      ))}
    </div>
  );
}
export function Vocabulary() {
  return (
    <div className="grid w-full grid-cols-2 gap-x-8 gap-y-4 sm:grid-cols-4 sm:gap-x-16">
      {(Object.keys(icons) as IconName[]).map((n) => (
        <div key={n} className="flex min-h-32 items-center gap-8 text-small text-fg-secondary">
          <Icon icon={icons[n]} size="sm" className="text-fg" />
          <span className="mono min-w-0 text-caption wrap-anywhere sm:text-small">{n}</span>
        </div>
      ))}
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Sizes">
        <Sizes />
      </StateSection>
      <StateSection title="Vocabulary">
        <Vocabulary />
      </StateSection>
    </>
  );
}
