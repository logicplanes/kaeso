import { Autocomplete, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

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
export function States() {
  return (
    <StateSection title="States">
      <div className="flex w-320 flex-col gap-12">
        <Autocomplete aria-label="Empty" suggestions={["/v1/responses"]} placeholder="/v1/…" />
        <Autocomplete aria-label="Focused" suggestions={["/v1/responses"]} defaultValue="/v1/res" data-force="focus" />
        <Autocomplete aria-label="Disabled" suggestions={["/v1/responses"]} disabled defaultValue="/v1/messages" />
      </div>
    </StateSection>
  );
}
