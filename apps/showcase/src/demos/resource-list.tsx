import { ResourceList, Avatar, Button, icons, type ListGroup } from "@logicplanes/kaeso-ui";
import { keys } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

const groups: ListGroup[] = [
  {
    id: "active",
    title: "Active",
    items: keys
      .filter((k) => k.state === "active")
      .map((k) => ({
        id: k.id,
        title: k.name,
        subtitle: <span className="mono text-small">{k.redacted}</span>,
        meta: <span className="text-small text-fg-secondary">{k.access === "platform" ? "Platform" : "Inference"}</span>,
        avatar: <Avatar name={k.owner} size={20} decorative />,
      })),
  },
  {
    id: "inactive",
    title: "Disabled and expired",
    items: keys
      .filter((k) => k.state === "disabled" || k.state === "expired")
      .map((k) => ({
        id: k.id,
        title: k.name,
        subtitle: <span className="mono text-small">{k.redacted}</span>,
        avatar: <Avatar name={k.owner} size={20} decorative />,
      })),
  },
];

export const meta = {
  title: "ResourceList",
  description: "A searchable list with group headers, row actions and a selection bar.",
  group: "Blocks",
  entry: "resource-list",
  import: "ResourceList",
  source: "blocks/resource-list",
};
export const examples = [{ id: "keys", title: "Keys", fn: "Keys", minHeight: 420 }];
export function Keys() {
  return (
    <div className="w-full">
      <ResourceList
        aria-label="Keys"
        groups={groups}
        bulkActions={[{ label: "Disable", icon: icons.hide, onClick: () => undefined }]}
        emptyAction={<Button variant="primary">Create key</Button>}
      />
    </div>
  );
}
export function States() {
  return (
    <StateSection title="List and empty">
      <div className="flex w-960 flex-col gap-40">
        <Keys />
        <ResourceList
          aria-label="Empty"
          groups={[]}
          emptyTitle="No keys yet"
          emptyDescription="Create a key so an application can call the gateway."
          emptyAction={<Button variant="primary">Create key</Button>}
        />
      </div>
    </StateSection>
  );
}
