import { useState } from "react";
import { ModelPicker } from "@logicplanes/kaeso-ui/playground";
import { playgroundModels } from "~/fixtures/playground";
import { Matrix } from "~/lib/states";

export const meta = {
  title: "ModelPicker",
  description: "A model from the project's catalogue: provider mark, name and id, grouped by provider. The start of a comparison column's header.",
  group: "Playground",
  entry: "playground",
  import: "ModelPicker",
  barrel: false,
  keyboard: [
    ["Enter Space", "Opens the list"],
    ["Up Down", "Moves through models"],
    ["A–Z", "Jumps to a model by name"],
  ],
  a11y: ["A select named “Model” by default; each option reads the model's name."],
};
export const examples = [
  { id: "header", title: "In a column header", fn: "Basic", minHeight: 160 },
  { id: "default", title: "Raised", fn: "Raised" },
];
export function Basic() {
  const [model, setModel] = useState("claude-sonnet-4-5");
  return (
    <div className="flex h-52 w-full max-w-360 items-center pl-8 hairline-b">
      <ModelPicker value={model} onValueChange={setModel} options={playgroundModels} />
    </div>
  );
}
export function Raised() {
  return <ModelPicker appearance="default" size="md" defaultValue="gpt-5" options={playgroundModels} />;
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
          />
        </div>
      )}
    />
  );
}
