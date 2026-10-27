import { TagInput } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";
import { FIELD_STATES, StateGrid } from "~/lib/state-grid";

const email = (t: string) => (/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(t) ? true : `“${t}” is not an email address.`);

export const meta = {
  title: "TagInput",
  description: "Removable tags in a field: Enter, comma and paste split them; Backspace selects the last, a second press removes it.",
  group: "Components",
  entry: "input",
  import: "TagInput",
  keyboard: [
    ["Enter", "Adds the typed tag"],
    [",", "Adds the typed tag"],
    ["Backspace", "Selects the last tag from the start of the field; a second press removes it"],
    ["← →", "Moves the selection between tags from the start of the field"],
    ["Delete", "Removes the selected tag"],
    ["Escape", "Lets go of the selected tag"],
  ],
  a11y: [
    "Each tag's remove circle is named “Remove …”; after it, focus goes back to the field.",
    "Selecting and removing a tag is announced through a polite live region, so nothing goes silently.",
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
const TEAM = ["lin@northwind.example", "tomas@northwind.example"];

export function States() {
  return (
    <>
      <StateGrid
        title="States"
        kinds={[
          { label: "Empty", id: "empty" },
          { label: "With tags", id: "tags" },
        ]}
        states={FIELD_STATES.filter((s) => s.id !== "read-only")}
        render={(k, p) => (
          <div className="w-240">
            {k.id === "empty" ? (
              <TagInput aria-label="Invite members" placeholder="name@example.com" {...p} />
            ) : (
              <TagInput aria-label="Invite members" defaultValue={TEAM.slice(0, 1)} {...p} />
            )}
          </div>
        )}
      />
      <StateSection title="Wrapping, invalid and long">
        <div className="flex w-400 flex-col gap-16">
          <TagInput aria-label="Wrapping" defaultValue={[...TEAM, "mira@northwind.example", "jon@northwind.example"]} />
          <TagInput label="Invite members" defaultValue={["lin@northwind.example", "not-an-email"]} validate={email} />
          <TagInput aria-label="One long address" defaultValue={["procurement-approvals-emea@northwind-holdings.example"]} />
        </div>
      </StateSection>
      <StateSection title="In a 240px column">
        <div className="w-240">
          <TagInput label="Allowed domains" defaultValue={["northwind.example", "checkout.northwind.example"]} />
        </div>
      </StateSection>
    </>
  );
}
