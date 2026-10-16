import { Logo } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Logo",
  description:
    "The Kaeso wordmark, mark and lockup as React SVG in currentColor. Interfaces show the wordmark alone; the mark is for favicons, app icons and avatars, the lockup for brand artwork.",
  group: "Components",
  entry: "logo",
  import: "Logo",
  a11y: [
    "The accessible name is “Kaeso”. Inside a link, name the link (“Kaeso home”) and pass decorative.",
    "A linked logo needs a name for the destination, such as “Kaeso home”.",
  ],
};
export const examples = [
  { id: "wordmark", title: "Wordmark: every interface, top left", fn: "Wordmark" },
  { id: "mark", title: "Mark: favicons, app icons, avatars", fn: "Mark" },
  { id: "lockup", title: "Lockup: brand artwork outside interfaces", fn: "Lockup" },
];
export function Wordmark() {
  return <Logo.Wordmark size="nav" />;
}
export function Lockup() {
  return <Logo.Lockup size="md" />;
}
export function Mark() {
  return (
    <div className="flex items-end gap-24">
      {(["sm", "md", "lg", "xl"] as const).map((s) => (
        <Logo.Mark key={s} size={s} />
      ))}
      <Logo.Icon size="lg" />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Lockup sizes">
        <div className="flex flex-col items-start gap-16">
          <Logo.Lockup size="sm" />
          <Logo.Lockup size="md" />
          <Logo.Lockup size="lg" />
        </div>
      </StateSection>
      <StateSection title="Mark and wordmark">
        <div className="flex items-center gap-24">
          <Mark />
          <Logo.Wordmark />
        </div>
      </StateSection>
    </>
  );
}
