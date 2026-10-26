import { InputGroup, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";
import { FIELD_STATES, StateGrid } from "~/lib/state-grid";

export const meta = {
  title: "InputGroup",
  description: "Addons inside the pill: text, icons, a ghost circle or a key cap, sharing one focus halo.",
  group: "Components",
  entry: "input",
  import: "InputGroup",
  a11y: [
    "Addons are text the input's label should not repeat; buttons inside need their own names.",
    "invalid on the group also marks the input aria-invalid, and disabled turns off its input and buttons.",
  ],
};
export const examples = [{ id: "addons", title: "Addons", fn: "Addons" }];
export function Addons() {
  return (
    <div className="flex w-400 flex-col gap-12">
      <InputGroup>
        <InputGroup.Addon data-join>https://</InputGroup.Addon>
        <InputGroup.Input aria-label="Host" placeholder="kaeso.example.com" />
        <InputGroup.Addon>/v1</InputGroup.Addon>
      </InputGroup>
      <InputGroup trailingButton>
        <InputGroup.Input aria-label="Token" defaultValue="sk-kso-AbC123…wxyz" data-mono />
        <InputGroup.Button icon={icons.copy} aria-label="Copy" />
      </InputGroup>
      <InputGroup leadingIcon>
        <InputGroup.Addon icon={icons.search} />
        <InputGroup.Input aria-label="Search" placeholder="Search models" />
      </InputGroup>
    </div>
  );
}
export function States() {
  return (
    <>
      <StateGrid
        title="States"
        kinds={[
          { label: "Prefix and suffix", id: "url" },
          { label: "Button", id: "button" },
        ]}
        states={FIELD_STATES}
        render={(k, p) => {
          const { readOnly, ...group } = p as { readOnly?: boolean };
          return (
            <div className="w-240">
              {k.id === "url" ? (
                <InputGroup {...group} readOnly={readOnly}>
                  <InputGroup.Addon data-join>https://</InputGroup.Addon>
                  <InputGroup.Input aria-label="Host" defaultValue="kaeso.example.com" readOnly={readOnly} />
                  <InputGroup.Addon>/v1</InputGroup.Addon>
                </InputGroup>
              ) : (
                <InputGroup trailingButton {...group} readOnly={readOnly}>
                  <InputGroup.Input aria-label="Token" defaultValue="sk-kso-AbC123…wxyz" data-mono readOnly={readOnly} />
                  <InputGroup.Button icon={icons.copy} aria-label="Copy" />
                </InputGroup>
              )}
            </div>
          );
        }}
      />
      <StateSection title="Sizes">
        <div className="flex w-320 flex-col gap-12">
          {(["sm", "md", "lg", "xl"] as const).map((s) => (
            <InputGroup key={s} size={s} trailingButton>
              <InputGroup.Addon data-join>https://</InputGroup.Addon>
              <InputGroup.Input aria-label={`Host, ${s}`} defaultValue="kaeso.example.com" />
              <InputGroup.Button icon={icons.copy} aria-label="Copy" />
            </InputGroup>
          ))}
        </div>
      </StateSection>
    </>
  );
}
