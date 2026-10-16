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
    <StateSection title="Layouts">
      <div className="flex w-960 flex-col gap-40">
        <Split />
        <Auth />
      </div>
    </StateSection>
  );
}
