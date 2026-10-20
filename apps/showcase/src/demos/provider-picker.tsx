import { useState } from "react";
import { ProviderPicker, type ProviderPickerOption } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

const providers: ProviderPickerOption[] = [
  { id: "openai", name: "OpenAI", connected: true, hint: "2 keys" },
  { id: "anthropic", name: "Anthropic", connected: true, hint: "1 key" },
  { id: "deepseek", name: "DeepSeek" },
  { id: "groq", name: "Groq" },
  { id: "mistral", name: "Mistral" },
  { id: "openrouter", name: "OpenRouter" },
  { id: "togetherai", name: "Together AI" },
  { id: "xai", name: "SpaceX" },
];

export const meta = {
  title: "ProviderPicker",
  description: "Choosing a provider from any number of them: connected ones first with their key counts, then the rest.",
  group: "Kaeso",
  entry: "kaeso",
  import: "ProviderPicker",
  keyboard: [
    ["A–Z", "Searches by name or id"],
    ["Up Down", "Moves through providers"],
    ["Enter", "Chooses the active provider"],
  ],
  a11y: ["A search field over a listbox, with Connected and Other providers as groups; each option reads the provider's name and hint."],
};
export const examples = [
  { id: "list", title: "In a dialog step", fn: "List" },
  { id: "select", title: "In a toolbar", fn: "Select", minHeight: 420 },
];
export function List() {
  const [provider, setProvider] = useState<string | null>("openai");
  return (
    <div className="w-400 max-w-full">
      <ProviderPicker aria-label="Provider" providers={providers} value={provider} onValueChange={setProvider} />
    </div>
  );
}
export function Select() {
  const [provider, setProvider] = useState<string | null>(null);
  return (
    <ProviderPicker
      appearance="select"
      size="sm"
      aria-label="Provider"
      allLabel="All providers"
      providers={providers}
      value={provider}
      onValueChange={setProvider}
    />
  );
}
export function States() {
  return (
    <>
      <StateSection title="Nothing connected yet">
        <div className="w-400 max-w-full">
          <ProviderPicker aria-label="Provider" providers={providers.map((p) => ({ id: p.id, name: p.name }))} />
        </div>
      </StateSection>
      <StateSection title="Chosen, in a form">
        <div className="w-320">
          <ProviderPicker appearance="select" label="Provider" providers={providers} value="anthropic" />
        </div>
      </StateSection>
    </>
  );
}
