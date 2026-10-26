import { Empty, Button, InsetContent, Link } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Empty",
  description: "What is absent and the next step: a title, one sentence, one action.",
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
      description={
        <>
          Create a key so an application can call the gateway. <Link href="https://docs.kaeso.app">Read about keys</Link>
        </>
      }
      actions={<Button variant="primary">Create key</Button>}
    />
  );
}
export function Compact() {
  return <Empty size="compact" title="No requests match" description="Try a wider time range." />;
}
export function States() {
  return (
    <>
      <StateSection title="Sizes">
        <div className="flex w-560 flex-col gap-24">
          <Page />
          <Empty size="md" title="No models" description="Add a model before creating keys." actions={<Button variant="primary">Add model</Button>} />
          <Compact />
          <Empty size="compact" title="Couldn't load these rows" actions={<Button size="sm">Try again</Button>} />
        </div>
      </StateSection>
      <StateSection title="In a card">
        <div className="w-560 rounded-group bg-raised shadow-xs">
          <InsetContent>
            <Empty size="md" title="No pending invitations" description="Accepted, revoked and expired invitations are under All." />
          </InsetContent>
        </div>
      </StateSection>
    </>
  );
}
