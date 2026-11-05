import { Input, icons, type InputSize } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";
import { FIELD_STATES, StateGrid } from "~/lib/state-grid";

export const meta = {
  title: "Input",
  description: "A pill text field with the secondary material and a soft focus halo.",
  group: "Components",
  entry: "input",
  import: "Input",
  keyboard: [
    ["Tab", "Moves focus in and out"],
    ["Escape", "Clears a search"],
  ],
  a11y: [
    "Pass label and description and the input wraps itself in Field, wiring aria-describedby; invalid sets aria-invalid, and an error shows under the input.",
    "Placeholders are hints, never labels.",
    "Read-only fields stay selectable and focusable.",
    "type=search lets assistive technology announce a search field; its clear button is named “Clear search”, and a shortcut hint hides while the field has focus without moving anything.",
    "A secret or password is masked behind a circle named “Show key” or “Show password”; a revealed key is set in mono, and a password field announces Caps Lock once.",
  ],
};
export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  {
    id: "field",
    title: "With label and help",
    fn: "WithField",
    description: "An invalid value gets the danger ring and a message under the field that says what to enter.",
  },
  { id: "sizes", title: "Sizes", fn: "Sizes" },
  { id: "addons", title: "Icon, prefix and suffix", fn: "Addons" },
  { id: "readonly", title: "Read-only and disabled", fn: "ReadOnly" },
  {
    id: "search",
    title: "Search",
    fn: "Search",
    description: '`type="search"`: a leading icon, a clear circle when filled, and a shortcut hint. Escape clears.',
  },
  { id: "secret", title: "Provider key", fn: "Secret", description: "`secret`: masked, an eye to reveal, mono when revealed, never autocompleted." },
  { id: "password", title: "Password", fn: "Password", description: '`type="password"`: the same eye, password managers welcome, and a Caps Lock hint.' },
];
export function Basic() {
  return (
    <div className="w-320">
      <Input aria-label="Key name" placeholder="checkout-web" />
    </div>
  );
}
export function WithField() {
  return (
    <div className="flex w-360 flex-col gap-24">
      <Input label="Key name" description="Shown in logs and the audit log." placeholder="checkout-web" />
      <Input label="Monthly budget" defaultValue="-20" suffix="USD" align="end" error="Enter an amount of 0 or more." />
    </div>
  );
}
export function Sizes() {
  return (
    <div className="flex w-320 flex-col gap-12">
      {(["sm", "md", "lg", "xl"] as InputSize[]).map((s) => (
        <Input key={s} size={s} aria-label={`Size ${s}`} placeholder={`Size ${s}`} />
      ))}
    </div>
  );
}
export function Addons() {
  return (
    <div className="flex w-360 flex-col gap-12">
      <Input aria-label="Email" leadingIcon={icons.mail} placeholder="ada@northwind.example" />
      <Input aria-label="Gateway URL" prefix="https://" placeholder="kaeso.example.com" />
      <Input aria-label="Rate limit" suffix="/ min" defaultValue="600" align="end" />
    </div>
  );
}
export function ReadOnly() {
  return (
    <div className="flex w-320 flex-col gap-12">
      <Input aria-label="Project id" readOnly defaultValue="proj_01J9Z5A" />
      <Input aria-label="Disabled" disabled defaultValue="Managed by the organization" />
    </div>
  );
}
const INPUT_KINDS = [
  { label: "Empty", id: "empty" },
  { label: "Filled", id: "filled" },
];

export function States() {
  return (
    <>
      <StateGrid
        title="States"
        kinds={INPUT_KINDS}
        states={FIELD_STATES}
        render={(k, p) => (
          <div className="w-200">
            <Input aria-label="Key name" placeholder="checkout-web" {...(k.id === "filled" ? { defaultValue: "checkout-web" } : {})} {...p} />
          </div>
        )}
      />
      <StateGrid
        title="Content × states"
        kinds={[
          { label: "Prefix", id: "prefix" },
          { label: "Icon and suffix", id: "suffix" },
        ]}
        states={FIELD_STATES.filter((s) => ["rest", "focus", "disabled", "read-only"].includes(s.id))}
        render={(k, p) => (
          <div className="w-200">
            {k.id === "prefix" ? (
              <Input aria-label="Gateway URL" prefix="https://" defaultValue="kaeso.example.com" {...p} />
            ) : (
              <Input aria-label="Budget" leadingIcon={icons.budget} suffix="USD" align="end" defaultValue="1,200.00" {...p} />
            )}
          </div>
        )}
      />
      <StateSection title="Sizes">
        <div className="flex w-240 flex-col gap-12">
          {(["sm", "md", "lg", "xl"] as InputSize[]).map((s) => (
            <Input key={s} size={s} aria-label={`Size ${s}`} prefix="https://" defaultValue="kaeso.example.com" />
          ))}
        </div>
      </StateSection>
      <StateSection title="Field">
        <div className="flex w-560 flex-col gap-24">
          <Input label="Key name" description="Shown in logs and the audit log." defaultValue="checkout-web" />
          <Input
            label="Monthly budget"
            description="Requests stop once this is spent."
            error="Enter an amount of 0 or more."
            defaultValue="-20"
            suffix="USD"
            align="end"
          />
          <Input label="Description" optional placeholder="What this key is for" />
          <Input label="Rate limit" layout="horizontal" description="Requests per minute for this key." defaultValue="600" suffix="/ min" />
          <Input label="Origin" defaultValue="https://checkout.northwind.example/api/v2/callbacks/payment-confirmation" />
        </div>
      </StateSection>
      <SearchStates />
      <StateSection title="Search in the settings sidebar">
        <SearchSettings />
      </StateSection>
      <MaskedStates />
    </>
  );
}

export function Search() {
  return (
    <div className="w-240">
      <Input type="search" size="sm" aria-label="Search logs" placeholder="gpt-5, req_01…" shortcut={["mod", "k"]} />
    </div>
  );
}
function SearchSettings() {
  return (
    <div className="w-240">
      <Input type="search" size="md" aria-label="Search settings" placeholder="Rate limit, budget" />
    </div>
  );
}
function SearchStates() {
  return (
    <>
      <StateGrid
        title="Search × states"
        kinds={[
          { label: "Empty, with shortcut", id: "empty" },
          { label: "Filled", id: "filled" },
        ]}
        states={FIELD_STATES.filter((s) => s.id !== "invalid")}
        render={(k, p) => (
          <div className="w-200">
            <Input
              type="search"
              size="sm"
              aria-label="Search logs"
              placeholder="gpt-5, req_01…"
              shortcut={["mod", "k"]}
              {...(k.id === "filled" ? { defaultValue: "gpt-5" } : {})}
              {...p}
            />
          </div>
        )}
      />
      <StateSection title="Search sizes">
        <div className="flex w-240 flex-col gap-12">
          {(["sm", "md", "lg", "xl"] as const).map((s) => (
            <Input type="search" key={s} size={s} aria-label={`Search, ${s}`} placeholder="gpt-5, req_01…" />
          ))}
        </div>
      </StateSection>
    </>
  );
}

export function Secret() {
  return (
    <div className="w-400">
      <Input secret label="Provider key" description="Only the last four characters stay visible after saving." placeholder="sk-ant-…" />
    </div>
  );
}
export function Password() {
  return (
    <div className="w-320">
      <Input type="password" label="Password" />
    </div>
  );
}
function MaskedStates() {
  return (
    <>
      <StateGrid
        title="Secret × states"
        kinds={[
          { label: "Masked", id: "masked" },
          { label: "Revealed", id: "revealed" },
        ]}
        states={FIELD_STATES}
        render={(k, p) => (
          <div className="w-240">
            <Input secret aria-label="Provider key" defaultValue="sk-ant-api03-Xf7Lq2" defaultRevealed={k.id === "revealed"} {...p} />
          </div>
        )}
      />
      <StateSection title="Password, with a description">
        <div className="flex w-320 flex-col gap-16">
          <Input type="password" label="Password" description="At least 12 characters." defaultValue="correct horse" />
          <Input secret label="Provider key" description="Only the last four characters stay visible after saving." placeholder="sk-ant-…" />
        </div>
      </StateSection>
    </>
  );
}
