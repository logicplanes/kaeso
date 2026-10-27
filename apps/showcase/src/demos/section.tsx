import { Section, SplitView, AuthLayout, Button, Input, Select, MetricRow, Stat } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Section",
  description: "Section, SplitView and AuthLayout: page structure with space instead of boxes.",
  group: "Layout",
  entry: "section",
  import: "Section, SplitView, AuthLayout",
};
export const examples = [
  { id: "split", title: "SplitView", fn: "Split" },
  { id: "auth", title: "AuthLayout", fn: "Auth", minHeight: 640 },
];
export function Split() {
  return (
    <div className="w-full">
      <SplitView
        aside={
          <>
            <Select
              label="Measure"
              options={[
                { value: "spend", label: "Spend" },
                { value: "requests", label: "Requests" },
              ]}
              defaultValue="spend"
              fullWidth
            />
            <Select
              label="Group by"
              options={[
                { value: "model", label: "Model" },
                { value: "key", label: "Key" },
              ]}
              defaultValue="model"
              fullWidth
            />
          </>
        }
      >
        <Section title="Usage" description="Last 30 days">
          <MetricRow>
            <Stat value="12,480" unit="requests" />
            <Stat value="$184.20" unit="total spend" />
          </MetricRow>
        </Section>
      </SplitView>
    </div>
  );
}
export function Auth() {
  return (
    <div className="h-640 w-full overflow-hidden rounded-frame shadow-frame">
      <AuthLayout title="Sign in to Kaeso" description="Use the account your administrator created." className="min-h-full">
        <Input label="Email" size="xl" placeholder="ada@northwind.example" />
        <Input label="Password" size="xl" type="password" />
        <Button variant="primary" size="xl" fullWidth>
          Sign in
        </Button>
      </AuthLayout>
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Sections: with actions, and without outer spacing">
        <div className="flex w-640 flex-col">
          <Section
            title="Members"
            description="People in the organization and their rights in it."
            actions={
              <Button size="sm" variant="secondary">
                Invite member
              </Button>
            }
          >
            <p className="text-body text-fg-secondary">5 members</p>
          </Section>
          <Section
            title="Mitglieder und Rechte in dieser Organisation"
            description="Wer sich anmelden darf und was jede Person sieht."
            actions={<Button size="sm">Einladen</Button>}
          >
            <p className="text-body text-fg-secondary">5 Mitglieder</p>
          </Section>
          <Section title="No outer spacing" spacing="none">
            <p className="text-body text-fg-secondary">Sits directly under the section above.</p>
          </Section>
        </div>
      </StateSection>
      <StateSection title="Layouts">
        <div className="flex w-640 flex-col gap-40">
          <Split />
          <Auth />
        </div>
      </StateSection>
    </>
  );
}
