import { useState } from "react";
import { Button, Input, KeyValue, LayerDialog, List, ModelLabel, SearchInput, icons, toast } from "@logicplanes/kaeso-ui";
import { models, providerNames } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "LayerDialog",
  description: "A second layer slides over the first (add model → review values); Back returns.",
  group: "Components",
  entry: "dialog",
  import: "LayerDialog",
  keyboard: [
    ["Escape", "Closes the dialog from any layer and returns focus to the trigger"],
    ["Tab", "Moves within the layer shown"],
    ["mod Enter", "Calls onSubmit, on every layer"],
  ],
  a11y: [
    "The layers below the one shown stay mounted but hidden: they keep their state, and the focus never reaches them.",
    "Moving to a layer puts the focus in it by the dialog's rule: the control marked data-autofocus, the first field, the primary or safe action, the first control, then Close. Never on the dialog box or the page, so ⌘/Ctrl+Enter keeps reaching onSubmit.",
    "Back returns the focus to the control that opened the layer, such as the row or the button, and the layer below to where it was scrolled; when that control is gone, the focus goes to the first control of the layer shown by the same rule.",
    "A new layer starts at its top. The title and description follow the layer shown and name the dialog.",
  ],
};
export const examples = [{ id: "wizard", title: "Add a model", fn: "Wizard" }];
export function Wizard() {
  const [open, setOpen] = useState(false);
  const [view, setView] = useState<"pick" | "review" | "custom">("pick");
  const [model, setModel] = useState("");
  const [custom, setCustom] = useState("");
  const [query, setQuery] = useState("");
  const m = models.find((x) => x.model === model);
  const start = () => {
    setView("pick");
    setModel("");
    setCustom("");
    setQuery("");
    setOpen(true);
  };
  const close = () => setOpen(false);
  const add = (id: string) => {
    close();
    toast.success(`Added ${id}`);
  };
  const addCustom = () => custom.trim() && add(custom.trim());
  const pick = {
    title: "Add model",
    description: "Pick a model from the KPMR catalogue.",
    toolbar: <SearchInput aria-label="Search models" placeholder="Search" value={query} onChange={(e) => setQuery(e.target.value)} data-autofocus />,
    content: (
      <div className="flex flex-col gap-12">
        <List
          aria-label="Models in KPMR"
          density="compact"
          items={models
            .filter((x) => x.model.includes(query.trim().toLowerCase()))
            .map((x) => ({
              id: x.model,
              textValue: x.model,
              title: <ModelLabel provider={x.provider} providerName={providerNames[x.provider]} model={x.model} name={x.name} />,
            }))}
          onOpen={(item) => {
            setModel(item.id);
            setView("review");
          }}
        />
      </div>
    ),
    footerStart: (
      <Button variant="ghost" leadingIcon={icons.add} onClick={() => setView("custom")}>
        Add a custom model
      </Button>
    ),
  };
  const second = {
    review: {
      title: `Review ${model}`,
      description: "Prices come from KPMR; you can override them.",
      content: m && (
        <div className="flex flex-col gap-16">
          <ModelLabel provider={m.provider} providerName={providerNames[m.provider]} model={m.model} name={m.name} capabilities={m.capabilities} />
          <KeyValue
            items={[
              { label: "Input", value: `$${m.input} / M` },
              { label: "Output", value: `$${m.output} / M` },
              { label: "Context", value: `${m.context.toLocaleString("en-US")} tokens` },
            ]}
          />
        </div>
      ),
      footer: (
        <Button variant="primary" onClick={() => add(model)}>
          Add model
        </Button>
      ),
    },
    custom: {
      title: "Add a custom model",
      description: "The id the provider knows the model by.",
      content: <Input label="Model id" placeholder="acme-internal-1" value={custom} onChange={(e) => setCustom(e.target.value)} />,
      footer: (
        <Button variant="primary" disabled={!custom.trim()} onClick={addCustom}>
          Add model
        </Button>
      ),
    },
  };
  return (
    <>
      <Button variant="primary" onClick={start}>
        Add model
      </Button>
      <LayerDialog
        open={open}
        onOpenChange={(o) => !o && close()}
        layer={view === "pick" ? 0 : 1}
        onLayerChange={(l) => l === 0 && setView("pick")}
        onSubmit={view === "review" ? () => add(model) : view === "custom" ? addCustom : undefined}
        layers={view === "pick" ? [pick] : [pick, second[view]]}
      />
    </>
  );
}
export function States() {
  const m = models.find((x) => x.model === "gpt-5-mini")!;
  const layers = [
    {
      title: "Add model",
      description: "Pick a model from the KPMR catalogue.",
      content: (
        <List
          aria-label="Models in KPMR"
          density="compact"
          items={models.map((x) => ({
            id: x.model,
            textValue: x.model,
            title: <ModelLabel provider={x.provider} providerName={providerNames[x.provider]} model={x.model} name={x.name} />,
          }))}
        />
      ),
    },
    {
      title: `Review ${m.model}`,
      description: "Prices come from KPMR; you can override them.",
      content: (
        <div className="flex flex-col gap-16">
          <ModelLabel provider={m.provider} providerName={providerNames[m.provider]} model={m.model} name={m.name} capabilities={m.capabilities} />
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
  const searched = [
    {
      ...layers[0]!,
      toolbar: <SearchInput aria-label="Search models" placeholder="Search" defaultValue="claude" />,
      footerStart: (
        <Button variant="ghost" leadingIcon={icons.add}>
          Add a custom model
        </Button>
      ),
    },
    layers[1]!,
  ];
  return (
    <>
      <StateSection title="First layer and second layer (with back)">
        <div className="flex flex-col gap-24">
          <LayerDialog inline layers={layers} layer={0} />
          <LayerDialog inline layers={layers} layer={1} />
        </div>
      </StateSection>
      <StateSection title="Toolbar and footer start, busy">
        <div className="flex flex-col gap-24">
          <LayerDialog inline layers={searched} layer={0} />
          <LayerDialog inline busy layers={layers} layer={1} />
        </div>
      </StateSection>
    </>
  );
}
