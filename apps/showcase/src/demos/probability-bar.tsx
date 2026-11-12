import { ProbabilityBar } from "@logicplanes/kaeso-ui/playground";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "ProbabilityBar",
  description: "One probability as a bar beside its label and its value in percent, the chosen one marked; a value the provider left out reads “Not given”.",
  group: "Playground",
  entry: "playground",
  import: "ProbabilityBar",
  barrel: false,
  a11y: [
    "Each bar is a meter named after its label; its value is read as text, such as “88%, chosen”.",
    "The chosen bar is marked by weight and in words, not by colour alone.",
    "A probability left out has no meter: its label and “Not given” read as plain text.",
    "The bar grows only without reduced motion, and a waiting bar pulses only without it.",
  ],
};
export const examples = [
  { id: "options", title: "A choice's options", fn: "Options" },
  { id: "stacked", title: "One probability", description: "The label and the value above the bar.", fn: "Stacked" },
];

export function Options() {
  return (
    <div className="flex w-full max-w-400 flex-col gap-4">
      <ProbabilityBar label="finance" value={0.8812} highlight />
      <ProbabilityBar label="legal" value={0.0041} />
      <ProbabilityBar label="support" value={0.1105} />
      <ProbabilityBar label="sales" value={null} />
    </div>
  );
}
export function Stacked() {
  return (
    <div className="w-full max-w-320">
      <ProbabilityBar layout="stacked" label="The document is an invoice" value={0.9731} />
    </div>
  );
}
export function States() {
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="A probability">
        <ProbabilityBar label="support" value={0.1105} />
      </StateSection>
      <StateSection title="Chosen">
        <ProbabilityBar label="finance" value={0.8812} highlight />
      </StateSection>
      <StateSection title="Under 1% and over 99%">
        <div className="flex flex-col gap-4">
          <ProbabilityBar label="legal" value={0.0041} />
          <ProbabilityBar label="Yes" value={0.9987} highlight />
        </div>
      </StateSection>
      <StateSection title="Not given">
        <ProbabilityBar label="sales" value={null} />
      </StateSection>
      <StateSection title="Waiting">
        <ProbabilityBar label="finance" pending />
      </StateSection>
      <StateSection title="A long label">
        <ProbabilityBar label="Contracts, data protection requests and anything a lawyer should read" value={0.42} />
      </StateSection>
      <StateSection title="Stacked">
        <ProbabilityBar layout="stacked" label="The document is an invoice" value={0.9731} />
      </StateSection>
    </div>
  );
}
