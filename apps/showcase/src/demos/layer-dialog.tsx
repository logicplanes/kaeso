import { useState } from "react";
import { Button, LayerDialog, Combobox, KeyValue, ModelLabel } from "@logicplanes/kaeso-ui";
import { models } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "LayerDialog",
  description: "A second layer slides over the first (add model → review values); Back returns.",
  group: "Components",
  entry: "dialog",
  import: "LayerDialog",
  keyboard: [["Escape", "Closes"]],
};
export const examples = [{ id: "wizard", title: "Add a model", fn: "Wizard" }];
export function Wizard() {
  const [layer, setLayer] = useState(0);
  const [model, setModel] = useState<string | null>("claude-haiku-4-5");
  const m = models.find((x) => x.model === model);
  return (
    <LayerDialog
      trigger={<Button variant="primary">Add model</Button>}
      layer={layer}
      onLayerChange={setLayer}
      onOpenChange={(o) => !o && setLayer(0)}
      layers={[
        {
          title: "Add model",
          description: "Search the models.dev catalogue.",
          content: (
            <Combobox
              aria-label="Model"
              options={models.map((x) => ({ value: x.model, label: x.model, description: x.name }))}
              value={model}
              onValueChange={setModel}
            />
          ),
          footer: (
            <Button variant="primary" disabled={!model} onClick={() => setLayer(1)}>
              Review values
            </Button>
          ),
        },
        {
          title: "Review values",
          description: "Prices come from models.dev; you can override them.",
          content: m && (
            <div className="flex flex-col gap-16">
              <ModelLabel provider={m.provider} model={m.model} name={m.name} capabilities={m.capabilities} />
              <KeyValue
                items={[
                  { label: "Input", value: `$${m.input} / M` },
                  { label: "Output", value: `$${m.output} / M` },
                  { label: "Context", value: `${m.context.toLocaleString("en-US")} tokens` },
                ]}
              />
            </div>
          ),
          footer: <Button variant="primary">Add model</Button>,
        },
      ]}
    />
  );
}
export function States() {
  const m = models.find((x) => x.model === "gpt-5-mini")!;
  const layers = [
    {
      title: "Add model",
      description: "Search the models.dev catalogue.",
      content: <Combobox aria-label="Model" options={models.map((x) => ({ value: x.model, label: x.model, description: x.name }))} defaultValue="gpt-5-mini" />,
      footer: <Button variant="primary">Review values</Button>,
    },
    {
      title: "Review values",
      description: "Prices come from models.dev; you can override them.",
      content: (
        <div className="flex flex-col gap-16">
          <ModelLabel provider={m.provider} model={m.model} name={m.name} capabilities={m.capabilities} />
          <KeyValue
            items={[
              { label: "Input", value: `$${m.input} / M` },
              { label: "Output", value: `$${m.output} / M` },
              { label: "Context", value: `${m.context.toLocaleString("en-US")} tokens` },
            ]}
          />
        </div>
      ),
      footer: <Button variant="primary">Add model</Button>,
    },
  ];
  return (
    <StateSection title="First layer and second layer (with back)">
      <div className="flex flex-col gap-24">
        <LayerDialog inline layers={layers} layer={0} />
        <LayerDialog inline layers={layers} layer={1} />
      </div>
    </StateSection>
  );
}
