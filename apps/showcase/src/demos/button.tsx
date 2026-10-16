import { useState } from "react";
import { Button, Inline, icons, KAESO_BUTTON_VARIANTS, type ButtonVariant, type ButtonSize } from "@logicplanes/kaeso-ui";
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
    "Icon-only buttons are circles and require aria-label at the type level; use IconButton, which also shows the name as a tooltip.",
    "Loading sets aria-busy at once and keeps the button focusable; clicks are ignored until it finishes. The spinner shows once the work has taken 300ms, for at least 400ms. The width never changes.",
    "With disabledReason, the button uses aria-disabled instead of disabled so keyboard users can reach the tooltip that explains why.",
    "Primary and danger labels measure at least 4.5:1 on their fills in both themes.",
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
  { id: "icon-only", title: "Icon only", fn: "IconOnly", description: "A circle of the same height. Prefer IconButton, which adds the tooltip." },
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
    <Inline gap={12}>
      <Button variant="primary">Create key</Button>
      <Button variant="primary" leadingIcon={icons.add}>
        Add model
      </Button>
    </Inline>
  );
}

export function Secondary() {
  return (
    <Inline gap={12}>
      <Button>Edit limits</Button>
      <Button leadingIcon={icons.download}>Export CSV</Button>
      <Button trailingIcon={icons.chevronDown}>Last 30 days</Button>
    </Inline>
  );
}

export function Ghost() {
  return (
    <Inline gap={12}>
      <Button variant="ghost">Cancel</Button>
      <Button variant="ghost" leadingIcon={icons.add}>
        Add filter
      </Button>
    </Inline>
  );
}

export function Soft() {
  return (
    <Inline gap={12}>
      <Button variant="soft" size="sm">
        Errors only
      </Button>
      <Button variant="soft" size="sm" leadingIcon={icons.filter}>
        Filter
      </Button>
    </Inline>
  );
}

export function Danger() {
  return (
    <Inline gap={12}>
      <Button variant="danger-secondary">Archive project</Button>
      <Button variant="danger">Delete key</Button>
    </Inline>
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
    <Inline gap={12} align="center">
      <Button size="xs">Extra small</Button>
      <Button size="sm">Small</Button>
      <Button size="md">Medium</Button>
      <Button size="lg">Large</Button>
      <Button size="xl">Extra large</Button>
    </Inline>
  );
}

export function WithIcons() {
  return (
    <Inline gap={12}>
      <Button variant="primary" leadingIcon={icons.key}>
        Create key
      </Button>
      <Button leadingIcon={icons.copy}>Copy key</Button>
      <Button trailingIcon={icons.externalLink}>Open docs</Button>
    </Inline>
  );
}

export function IconOnly() {
  return (
    <Inline gap={12}>
      <Button shape="circle" icon={icons.filter} aria-label="Filter" />
      <Button shape="circle" variant="ghost" icon={icons.more} aria-label="More actions" />
      <Button shape="circle" variant="primary" icon={icons.add} aria-label="Create key" />
    </Inline>
  );
}

export function Loading() {
  const [saving, setSaving] = useState(false);
  return (
    <Inline gap={12}>
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
    </Inline>
  );
}

export function Disabled() {
  return (
    <Inline gap={12}>
      <Button variant="primary" disabled>
        Create key
      </Button>
      <Button disabled disabledReason="Add a provider key first">
        Add model
      </Button>
    </Inline>
  );
}

export function CountShortcut() {
  return (
    <Inline gap={12}>
      <Button count={12}>Requests</Button>
      <Button size="lg" variant="primary" shortcut={["mod", "enter"]}>
        Create key
      </Button>
    </Inline>
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
    <Inline gap={8}>
      <Button variant="ghost">Cancel</Button>
      <Button variant="primary">Create key</Button>
    </Inline>
  );
}

export function DontTwoPrimaries() {
  return (
    <Inline gap={8}>
      <Button variant="primary">Cancel</Button>
      <Button variant="primary">Create key</Button>
    </Inline>
  );
}

const VARIANTS = Object.keys(KAESO_BUTTON_VARIANTS.variant) as ButtonVariant[];
const SIZES = Object.keys(KAESO_BUTTON_VARIANTS.size) as ButtonSize[];
const STATES: [string, Record<string, unknown>][] = [
  ["Rest", {}],
  ["Hover", { "data-force": "hover" }],
  ["Focus", { "data-force": "focus" }],
  ["Pressed", { "data-force": "pressed" }],
  ["Open", { "data-force": "open" }],
  ["Disabled", { disabled: true }],
  ["Loading", { loading: true }],
];

export function States() {
  return (
    <>
      <StateMatrix title="Variants × states" columns={STATES.map(([s]) => s)}>
        {VARIANTS.map((v) => (
          <StateRow key={v} label={v}>
            {STATES.map(([s, p]) => (
              <StateCell key={s} id={`${v}-${s.toLowerCase()}`}>
                <Button variant={v} {...p}>
                  Create key
                </Button>
              </StateCell>
            ))}
          </StateRow>
        ))}
      </StateMatrix>
      <StateMatrix title="Sizes × content" columns={["Label", "Leading icon", "Trailing icon", "Circle", "Count", "Loading"]}>
        {SIZES.map((s) => (
          <StateRow key={s} label={s}>
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
            <StateCell>
              <Button size={s} shape="circle" icon={icons.filter} aria-label="Filter" />
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
          </StateRow>
        ))}
      </StateMatrix>
      <StateSection title="Long labels and full width">
        <div className="flex w-320 flex-col gap-12">
          <Button fullWidth variant="primary">
            Create key for production
          </Button>
          <div className="w-160">
            <Button>Read the gateway setup guide</Button>
          </div>
        </div>
      </StateSection>
    </>
  );
}
