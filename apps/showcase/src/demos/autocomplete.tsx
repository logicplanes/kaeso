import { Autocomplete, icons } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "Autocomplete",
  description: "Free text with suggestions.",
  group: "Components",
  entry: "combobox",
  import: "Autocomplete",
  keyboard: [
    ["Up Down", "Moves through suggestions"],
    ["Enter", "Takes a suggestion"],
  ],
};
export const examples = [{ id: "basic", title: "Basic", fn: "Basic" }];
export function Basic() {
  return (
    <div className="w-320">
      <Autocomplete
        label="Endpoint"
        leadingIcon={icons.route}
        suggestions={["/v1/responses", "/v1/chat/completions", "/v1/messages", "/v1/embeddings"]}
        placeholder="/v1/…"
      />
    </div>
  );
}
const paths = ["/v1/responses", "/v1/chat/completions", "/v1/messages", "/v1/embeddings"];

function Field(props: Record<string, unknown>) {
  return (
    <div className="w-140">
      <Autocomplete aria-label="Path" suggestions={paths} defaultValue="/v1/responses" placeholder="/v1/…" {...props} />
    </div>
  );
}

export function States() {
  return (
    <>
      <Matrix
        title="Field"
        rows={[{ label: "value" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Disabled", props: { disabled: true } },
        ]}
        render={(_, p) => <Field {...p} />}
      />
      <Matrix title="Focus" rows={[{ label: "value" }]} columns={[{ label: "Focus", props: { "data-force": "focus" } }]} render={(_, p) => <Field {...p} />} />
      <StateSection title="Icons, sizes and errors">
        <div className="flex w-320 flex-col gap-12">
          <Autocomplete aria-label="Empty" suggestions={paths} placeholder="/v1/…" />
          {(["sm", "md", "lg", "xl"] as const).map((size) => (
            <Autocomplete key={size} aria-label={`Path, ${size}`} size={size} leadingIcon={icons.search} suggestions={paths} placeholder="Filter by path" />
          ))}
          <Autocomplete label="Path" suggestions={paths} defaultValue="v1/responses" error="Start the path with a slash: /v1/responses." />
        </div>
      </StateSection>
    </>
  );
}
