import { useState } from "react";
import { ModelPicker } from "@logicplanes/kaeso-ui/playground";
import { playgroundModels } from "~/fixtures/playground";
import { models, providers } from "~/fixtures/kaeso";

const catalogue = models.map((m) => ({
  value: m.model,
  provider: m.provider,
  name: m.name,
  capabilities: m.capabilities,
  context: m.context,
  price: `$${m.input} / $${m.output}`,
}));
import { Matrix } from "~/lib/states";

export const meta = {
  title: "ModelPicker",
  description: "A model from a catalogue of any size: searchable, grouped by provider, recent models on top. The start of a comparison column's header.",
  group: "Playground",
  entry: "playground",
  import: "ModelPicker",
  barrel: false,
  keyboard: [
    ["Enter Space", "Opens the list with its search focused"],
    ["A–Z", "Searches by name, id or provider"],
    ["Up Down", "Moves through models"],
    ["Enter", "Chooses the active model"],
    ["Escape", "Clears the search, then closes"],
  ],
  a11y: [
    "A button named “Model” by default opens a search field over a listbox, one group per provider.",
    "Above the list, a Provider select narrows it to one provider; it offers only the providers whose models match the search.",
    "On phones the list opens as a full-screen sheet with the search on top.",
  ],
};
export const examples = [
  { id: "header", title: "In a column header", fn: "Basic", minHeight: 160 },
  { id: "default", title: "Raised", fn: "Raised" },
  { id: "catalogue", title: "With details and recent models", fn: "Catalogue", minHeight: 560 },
];
export function Basic() {
  const [model, setModel] = useState("claude-sonnet-4-5");
  return (
    <div className="flex h-52 w-full max-w-360 items-center pl-8 hairline-b">
      <ModelPicker value={model} onValueChange={setModel} options={playgroundModels} providers={providers} />
    </div>
  );
}
export function Raised() {
  return <ModelPicker appearance="default" size="md" defaultValue="gpt-5" options={playgroundModels} providers={providers} />;
}
export function Catalogue() {
  const [model, setModel] = useState<string | null>(null);
  return (
    <ModelPicker
      appearance="default"
      size="md"
      options={catalogue}
      providers={providers.map((p) => ({ ...p, connected: p.id === "anthropic" }))}
      recent={["claude-sonnet-4-5"]}
      value={model}
      onValueChange={setModel}
    />
  );
}
export function States() {
  return (
    <Matrix
      title="Appearance"
      rows={[{ label: "Ghost" }, { label: "Raised" }, { label: "Empty" }]}
      render={(row, props) => (
        <div className="w-280">
          <ModelPicker
            {...(props as object)}
            appearance={row.label === "Raised" ? "default" : "ghost"}
            {...(row.label === "Empty" ? {} : { defaultValue: "gpt-5-mini" })}
            options={playgroundModels}
            providers={providers}
          />
        </div>
      )}
    />
  );
}
