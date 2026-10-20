import { TagInput } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

const email = (t: string) => (/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(t) ? true : `“${t}” is not an email address.`);

export const meta = {
  title: "TagInput",
  description: "Removable tags in a field: Enter, comma and paste split them; Backspace removes the last.",
  group: "Components",
  entry: "input",
  import: "TagInput",
  keyboard: [
    ["Enter", "Adds the typed tag"],
    [",", "Adds the typed tag"],
    ["Backspace", "Removes the last tag when the field is empty"],
  ],
  a11y: [
    "Each tag's remove circle is named “Remove …”.",
    "Invalid tags stay, marked with the danger ring; each says what is wrong in its tooltip, and the first problem shows under the field.",
  ],
};
export const examples = [{ id: "invite", title: "Invite by email", fn: "Invite" }];
export function Invite() {
  return (
    <div className="w-400">
      <TagInput label="Invite members" placeholder="name@example.com" defaultValue={["lin@northwind.example", "tomas@northwind.example"]} validate={email} />
    </div>
  );
}
export function States() {
  return (
    <StateSection title="States">
      <div className="flex w-400 flex-col gap-16">
        <TagInput aria-label="Empty" placeholder="name@example.com" />
        <TagInput aria-label="Focused" data-force="focus" defaultValue={["lin@northwind.example"]} />
        <TagInput
          aria-label="Wrapping"
          defaultValue={["lin@northwind.example", "tomas@northwind.example", "mira@northwind.example", "jon@northwind.example"]}
        />
        <TagInput aria-label="Invalid" defaultValue={["lin@northwind.example", "not-an-email"]} validate={email} />
        <TagInput aria-label="Disabled" disabled defaultValue={["lin@northwind.example"]} />
      </div>
    </StateSection>
  );
}
