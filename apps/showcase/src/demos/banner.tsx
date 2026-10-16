import { Banner, Button, PageHeader } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Banner",
  description: "A standing notice about a state: a tint, an icon, the text and actions; errors are toasts.",
  group: "Components",
  entry: "banner",
  import: "Banner",
  a11y: ["Danger banners are alerts; others are status regions.", "Only informational banners can be dismissed."],
};
export const examples = [
  { id: "tones", title: "Tones", fn: "Tones" },
  {
    id: "full",
    title: "Full width",
    description:
      "Across the top of the panel, its sides on the page's 24px inset: the action ends where the page header's actions end. On a phone it goes under the text.",
    fn: "Full",
  },
];
export function Tones() {
  return (
    <div className="flex w-560 max-w-full flex-col gap-12">
      <Banner tone="info" title="New models are available" actions={<Button size="sm">Review</Button>} onDismiss={() => undefined}>
        models.dev added 4 models since the last sync.
      </Banner>
      <Banner tone="warning">This key expires in 5 days.</Banner>
      <Banner tone="danger" title="This organization is scheduled for deletion" actions={<Button size="sm">Restore organization</Button>}>
        Its keys stopped working; on 20 Oct 2026 it is deleted for good.
      </Banner>
      <Banner tone="success">Single sign-on is on for northwind.example.</Banner>
      <Banner>Content logging is off for this project.</Banner>
    </div>
  );
}

function Deletion({ loading = false }: { loading?: boolean }) {
  return (
    <Banner
      layout="full"
      tone="danger"
      title="This organization is scheduled for deletion"
      actions={
        <Button size="sm" loading={loading}>
          Restore organization
        </Button>
      }
    >
      Its keys stopped working; on 20 Oct 2026 it is deleted for good.
    </Banner>
  );
}

function Panel({ loading = false }: { loading?: boolean }) {
  return (
    <div className="w-full rounded-frame bg-chrome p-8 shadow-frame">
      <div className="overflow-hidden rounded-panel bg-panel">
        <Deletion loading={loading} />
        <PageHeader title="Request logs" actions={<Button size="sm">Export</Button>} />
      </div>
    </div>
  );
}

export function Full() {
  return <Panel />;
}
export function States() {
  return (
    <>
      <StateSection title="Tones">
        <Tones />
      </StateSection>
      <StateSection title="Inline: text only, a title, actions, two actions, a long title">
        <div className="flex w-560 max-w-full flex-col gap-12">
          <Banner tone="warning" actions={<Button size="sm">Extend</Button>}>
            This key expires in 5 days.
          </Banner>
          <Banner tone="info" title="Usage is recorded from 1 Oct" onDismiss={() => undefined}>
            Earlier requests are in the request logs but not in the usage charts. Exports cover the same period.
          </Banner>
          <Banner
            tone="warning"
            title="Monthly budget at 90%"
            actions={
              <>
                <Button size="sm">View usage</Button>
                <Button size="sm" variant="primary">
                  Raise budget
                </Button>
              </>
            }
          >
            Checkout assistant has used $450 of $500 this month.
          </Banner>
          <Banner
            tone="danger"
            title="This organization is scheduled for deletion, along with its 3 projects, 11 keys and request logs"
            actions={<Button size="sm">Restore organization</Button>}
          >
            Its keys stopped working; on 20 Oct 2026 it is deleted for good.
          </Banner>
        </div>
      </StateSection>
      <StateSection title="Full width over a page header: rest, restoring (nothing moves)">
        <div className="flex w-800 max-w-full flex-col gap-24">
          <Panel />
          <Panel loading />
        </div>
      </StateSection>
      <StateSection title="Narrow (a phone): the action under the text, at its left edge">
        <div className="flex flex-wrap gap-24">
          <div className="w-360">
            <Panel />
          </div>
          <div className="w-360">
            <Banner tone="info" title="New models are available" actions={<Button size="sm">Review</Button>} onDismiss={() => undefined}>
              models.dev added 4 models since the last sync.
            </Banner>
          </div>
        </div>
      </StateSection>
    </>
  );
}
