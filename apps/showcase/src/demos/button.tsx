import { useState } from "react";
import { Button, Select, icons, KAESO_BUTTON_VARIANTS, type ButtonVariant, type ButtonSize } from "@logicplanes/kaeso-ui";
import { StateMatrix, StateRow, StateCell, StateSection } from "~/lib/states";

export const meta = {
  title: "Button",
  description: "Starts an action. Every button is a pill; exactly one per view is ink.",
  group: "Components",
  entry: "button",
  import: "Button",
  keyboard: [
    ["Enter", "Activates the button"],
    ["Space", "Activates the button"],
    ["Tab", "Moves focus to the next control; a disabled button with a reason stays focusable"],
  ],
  a11y: [
    'Renders a native <button type="button">, or an <a> when href is set.',
    "A button with an icon and no label is a circle. Its aria-label is required by the types and also shows as its tooltip, with a shortcut as key caps, so sighted and screen-reader users get the same name.",
    "Loading sets aria-busy at once and keeps the button focusable; clicks are ignored until it finishes. The spinner shows once the work has taken 300ms, for at least 400ms. The width never changes.",
    "With disabledReason, the button uses aria-disabled instead of disabled so keyboard users can reach the tooltip that explains why.",
    "Primary and danger labels measure at least 4.5:1 on their fills in both themes.",
    "With pressed, it is a toggle button (aria-pressed), named by its label; the selector's check is decoration.",
  ],
};

export const examples = [
  { id: "basic", title: "Basic", fn: "Basic" },
  { id: "variant-primary", title: "Primary", fn: "Primary", description: "Ink with a faint top light. Use it for the one action a view exists for." },
  { id: "variant-secondary", title: "Secondary", fn: "Secondary", description: "The default: raised, with a ring and a shadow you feel more than see." },
  { id: "variant-ghost", title: "Ghost", fn: "Ghost", description: "Transparent until touched; for toolbars, rows and quiet actions." },
  { id: "variant-soft", title: "Soft", fn: "Soft", description: "A fill without a ring, for the active pill in a header or toolbar." },
  { id: "variant-danger", title: "Danger", fn: "Danger", description: "The confirm of a destructive flow. Its trigger is usually danger-secondary." },
  { id: "variant-link", title: "Link", fn: "LinkVariant", description: "An action inside a sentence." },
  {
    id: "sizes",
    title: "Sizes",
    fn: "Sizes",
    description: "24 for chips inside rows, 32 in toolbars, 36 by default, 40 for prominent actions, 48 in sign-in and setup forms.",
  },
  { id: "icons", title: "With icons", fn: "WithIcons", description: "A leading icon takes 2px off its side's padding so the label looks centered." },
  { id: "icon-only", title: "Icon only", fn: "IconOnly", description: "A circle of the same height, with its name in a tooltip." },
  { id: "icon-shortcut", title: "Icon only, with a shortcut", fn: "IconWithShortcut", description: "The shortcut shows in the tooltip as key caps." },
  {
    id: "filter-toggle",
    title: "On-or-off filter",
    fn: "FilterToggle",
    description: '`variant="selector"` with `pressed`: quiet like an unset select when off, a check and full text when on.',
  },
  {
    id: "loading",
    title: "Loading",
    fn: "Loading",
    description:
      "Busy at once; the spinner shows once the work has taken 300ms, then for at least 400ms, replacing the leading icon or covering the label. The width never changes.",
  },
  { id: "disabled", title: "Disabled", fn: "Disabled", description: "Give a reason when you can: it shows as a tooltip and keeps the button focusable." },
  { id: "count-shortcut", title: "Count and shortcut", fn: "CountShortcut" },
  { id: "link", title: "As a link", fn: "AsLink", description: "With href the button renders a link through LinkProvider." },
  { id: "full-width", title: "Full width", fn: "FullWidth" },
];

export function Basic() {
  return <Button variant="primary">Create key</Button>;
}

export function Primary() {
  return (
    <div className="flex flex-wrap items-center gap-12">
      <Button variant="primary">Create key</Button>
      <Button variant="primary" leadingIcon={icons.add}>
        Add model
      </Button>
    </div>
  );
}

export function Secondary() {
  return (
    <div className="flex flex-wrap items-center gap-12">
      <Button>Edit limits</Button>
      <Button leadingIcon={icons.download}>Export CSV</Button>
      <Button trailingIcon={icons.chevronDown}>Last 30 days</Button>
    </div>
  );
}

export function Ghost() {
  return (
    <div className="flex flex-wrap items-center gap-12">
      <Button variant="ghost">Cancel</Button>
      <Button variant="ghost" leadingIcon={icons.add}>
        Add filter
      </Button>
    </div>
  );
}

export function Soft() {
  return (
    <div className="flex flex-wrap items-center gap-12">
      <Button variant="soft" size="sm">
        Errors only
      </Button>
      <Button variant="soft" size="sm" leadingIcon={icons.filter}>
        Filter
      </Button>
    </div>
  );
}

export function Danger() {
  return (
    <div className="flex flex-wrap items-center gap-12">
      <Button variant="danger-secondary">Archive project</Button>
      <Button variant="danger">Delete key</Button>
    </div>
  );
}

export function LinkVariant() {
  return (
    <p className="text-body text-fg-secondary">
      No keys yet.{" "}
      <Button variant="link" size="sm">
        Create the first key
      </Button>
    </p>
  );
}

export function Sizes() {
  return (
    <div className="flex flex-wrap items-center gap-12">
      <Button size="xs">Extra small</Button>
      <Button size="sm">Small</Button>
      <Button size="md">Medium</Button>
      <Button size="lg">Large</Button>
      <Button size="xl">Extra large</Button>
    </div>
  );
}

export function WithIcons() {
  return (
    <div className="flex flex-wrap items-center gap-12">
      <Button variant="primary" leadingIcon={icons.key}>
        Create key
      </Button>
      <Button leadingIcon={icons.copy}>Copy key</Button>
      <Button trailingIcon={icons.externalLink}>Open docs</Button>
    </div>
  );
}

export function IconOnly() {
  return (
    <div className="flex flex-wrap items-center gap-12">
      <Button icon={icons.filter} aria-label="Filter" />
      <Button variant="ghost" icon={icons.more} aria-label="More actions" />
      <Button variant="primary" icon={icons.add} aria-label="Create key" />
    </div>
  );
}

export function FilterToggle() {
  const [agents, setAgents] = useState(false);
  const [overridden, setOverridden] = useState(true);
  return (
    <div className="flex flex-wrap items-center gap-8">
      <Select size="sm" aria-label="Actor" placeholder="Any actor" options={[{ value: "ada", label: "Ada Okafor" }]} value={null} onValueChange={() => {}} />
      <Button variant="selector" size="sm" pressed={agents} onPressedChange={setAgents}>
        Agents
      </Button>
      <Button variant="selector" size="sm" pressed={overridden} onPressedChange={setOverridden}>
        Only overridden
      </Button>
    </div>
  );
}

export function IconWithShortcut() {
  return <Button size="sm" icon={icons.search} aria-label="Search" shortcut={["mod", "k"]} />;
}

export function Loading() {
  const [saving, setSaving] = useState(false);
  return (
    <div className="flex flex-wrap items-center gap-12">
      <Button
        variant="primary"
        loading={saving}
        onClick={() => {
          setSaving(true);
          setTimeout(() => setSaving(false), 1600);
        }}
      >
        Save changes
      </Button>
      <Button loading leadingIcon={icons.refresh}>
        Syncing models
      </Button>
    </div>
  );
}

export function Disabled() {
  return (
    <div className="flex flex-wrap items-center gap-12">
      <Button variant="primary" disabled>
        Create key
      </Button>
      <Button disabled disabledReason="Add a provider key first">
        Add model
      </Button>
    </div>
  );
}

export function CountShortcut() {
  return (
    <div className="flex flex-wrap items-center gap-12">
      <Button count={12}>Requests</Button>
      <Button size="lg" variant="primary" shortcut={["mod", "enter"]}>
        Create key
      </Button>
    </div>
  );
}

export function AsLink() {
  return (
    <Button href="https://ui.kaeso.app" trailingIcon={icons.externalLink}>
      Read the gateway setup guide
    </Button>
  );
}

export function FullWidth() {
  return (
    <div className="w-320">
      <Button variant="primary" size="xl" fullWidth>
        Sign in
      </Button>
    </div>
  );
}

export function DoOnePrimary() {
  return (
    <div className="flex flex-wrap items-center gap-8">
      <Button variant="ghost">Cancel</Button>
      <Button variant="primary">Create key</Button>
    </div>
  );
}

export function DontTwoPrimaries() {
  return (
    <div className="flex flex-wrap items-center gap-8">
      <Button variant="primary">Cancel</Button>
      <Button variant="primary">Create key</Button>
    </div>
  );
}

const VARIANTS = Object.keys(KAESO_BUTTON_VARIANTS.variant) as ButtonVariant[];
const SIZES = Object.keys(KAESO_BUTTON_VARIANTS.size) as ButtonSize[];
const STATE_SETS: [string, Record<string, unknown>][][] = [
  [
    ["Rest", {}],
    ["Hover", { "data-force": "hover" }],
    ["Focus", { "data-force": "focus" }],
    ["Pressed", { "data-force": "pressed" }],
  ],
  [
    ["Open", { "data-force": "open" }],
    ["Disabled", { disabled: true }],
    ["Soft-disabled", { disabled: true, disabledReason: "Ask an owner to create keys" }],
    ["Loading", { loading: true }],
  ],
];

export function States() {
  return (
    <>
      {STATE_SETS.map((states, i) => (
        <StateMatrix key={`icon-${i}`} title={i === 0 ? "Icon only × states" : "Icon only × more states"} columns={states.map(([s]) => s)}>
          {VARIANTS.filter((v) => v !== "link").map((v) => (
            <StateRow key={v} label={v}>
              {states.map(([s, p]) => (
                <StateCell key={s} id={`icon-${v}-${s.toLowerCase()}`}>
                  <Button variant={v} size="sm" icon={icons.filter} aria-label="Filter" tooltip={false} {...p} />
                </StateCell>
              ))}
            </StateRow>
          ))}
        </StateMatrix>
      ))}
      {STATE_SETS.map((states, i) => (
        <StateMatrix key={i} title={i === 0 ? "Variants × states" : "Variants × more states"} columns={states.map(([s]) => s)}>
          {VARIANTS.map((v) => (
            <StateRow key={v} label={v}>
              {states.map(([s, p]) => (
                <StateCell key={s} id={`${v}-${s.toLowerCase()}`}>
                  <Button variant={v} {...p}>
                    Add key
                  </Button>
                </StateCell>
              ))}
            </StateRow>
          ))}
        </StateMatrix>
      ))}
      <StateMatrix title="Variants × content" columns={["Shortcut", "Count", "As a link"]}>
        {VARIANTS.filter((v) => v !== "link").map((v) => (
          <StateRow key={v} label={v}>
            <StateCell>
              <Button variant={v} size="lg" shortcut={["mod", "enter"]}>
                Send
              </Button>
            </StateCell>
            <StateCell>
              <Button variant={v} count={3}>
                Logs
              </Button>
            </StateCell>
            <StateCell>
              <Button variant={v} href="/docs/keys">
                Open docs
              </Button>
            </StateCell>
          </StateRow>
        ))}
      </StateMatrix>
      {[
        ["Label", "Leading icon", "Trailing icon"],
        ["Circle", "Count", "Loading"],
      ].map((cols, i) => (
        <StateMatrix key={cols[0]} title={i === 0 ? "Sizes × content" : "Sizes × more content"} columns={cols}>
          {SIZES.map((s) => (
            <StateRow key={s} label={s}>
              {i === 0 ? (
                <>
                  <StateCell>
                    <Button size={s}>Save</Button>
                  </StateCell>
                  <StateCell>
                    <Button size={s} leadingIcon={icons.add}>
                      Add
                    </Button>
                  </StateCell>
                  <StateCell>
                    <Button size={s} trailingIcon={icons.chevronDown}>
                      Period
                    </Button>
                  </StateCell>
                </>
              ) : (
                <>
                  <StateCell>
                    <Button size={s} icon={icons.filter} aria-label="Filter" />
                  </StateCell>
                  <StateCell>
                    <Button size={s} count={8}>
                      Logs
                    </Button>
                  </StateCell>
                  <StateCell>
                    <Button size={s} variant="primary" loading leadingIcon={icons.add}>
                      Add
                    </Button>
                  </StateCell>
                </>
              )}
            </StateRow>
          ))}
        </StateMatrix>
      ))}
      <StateSection title="Inside a sentence">
        <div className="flex max-w-320 flex-col gap-12">
          <p className="text-body text-fg-secondary">
            No keys yet.{" "}
            <Button variant="link" size="sm">
              Create the first key
            </Button>{" "}
            to send a request.
          </p>
          <p className="text-body text-fg-secondary">
            No keys yet.{" "}
            <Button variant="link" size="sm" data-force="focus">
              Create the first key
            </Button>{" "}
            to send a request.
          </p>
        </div>
      </StateSection>
      <StateSection title="Long labels and narrow containers">
        <div className="flex w-320 flex-col gap-12">
          <Button fullWidth variant="primary">
            Create key for production
          </Button>
          <div className="w-160">
            <Button>Read the gateway setup guide</Button>
          </div>
          <div className="w-160">
            <Button variant="danger-secondary">Projektschlüssel endgültig löschen</Button>
          </div>
          <div className="w-160">
            <Button leadingIcon={icons.add} variant="primary">
              Dienstkonto hinzufügen
            </Button>
          </div>
        </div>
      </StateSection>
    </>
  );
}
