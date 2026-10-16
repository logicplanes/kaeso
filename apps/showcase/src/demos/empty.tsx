import { Empty, Button } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Empty",
  description: "What is absent and the next step: an icon circle, a title, one sentence, one or two actions.",
  group: "Components",
  entry: "empty",
  import: "Empty",
};
export const examples = [
  { id: "page", title: "In a page", fn: "Page" },
  { id: "compact", title: "In a table", fn: "Compact" },
];
export function Page() {
  return (
    <Empty
      title="No keys yet"
      description="Create a key so an application can call the gateway."
      actions={
        <>
          <Button variant="primary">Create key</Button>
          <Button>Read about keys</Button>
        </>
      }
    />
  );
}
export function Compact() {
  return <Empty size="compact" title="No requests match" description="Try a wider time range." />;
}
export function States() {
  return (
    <StateSection title="Sizes">
      <div className="flex w-640 flex-col gap-24">
        <Page />
        <Empty size="md" title="No models" description="Add a model before creating keys." actions={<Button variant="primary">Add model</Button>} />
        <Compact />
      </div>
    </StateSection>
  );
}
