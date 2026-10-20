import { SettingsGroup, SettingsRow, SettingsSection, Select, Switch, Button, Link, Input, InlineCopyText, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "SettingsGroup",
  description: "Settings rows in a rounded group, title and description left, the control right.",
  group: "Layout",
  entry: "settings",
  import: "SettingsSection, SettingsGroup, SettingsRow",
  a11y: ["Name controls in rows with aria-label or aria-labelledby pointing at the row title.", "Navigation rows are links; the whole row is the target."],
};
export const examples = [
  { id: "preferences", title: "Preferences", fn: "Preferences" },
  { id: "navigation", title: "Navigation rows", fn: "Navigation" },
  { id: "narrow", title: "Narrow column", fn: "Narrow" },
];
export function Preferences() {
  return (
    <div className="w-720">
      <SettingsSection title="General">
        <SettingsGroup>
          <SettingsRow
            title="Default project"
            description="Opens when you sign in."
            control={
              <Select
                appearance="ghost"
                size="sm"
                aria-label="Default project"
                options={[
                  { value: "a", label: "Checkout assistant" },
                  { value: "b", label: "Support triage" },
                ]}
                defaultValue="a"
              />
            }
          />
          <SettingsRow
            title="First day of the week"
            description="Used in date pickers and usage charts."
            control={
              <Select
                appearance="ghost"
                size="sm"
                aria-label="First day of the week"
                options={[
                  { value: "mon", label: "Monday" },
                  { value: "sun", label: "Sunday" },
                ]}
                defaultValue="mon"
              />
            }
          />
          <SettingsRow
            title="Show times in UTC"
            description="Request logs and the audit log use UTC instead of your time zone."
            control={<Switch aria-label="Show times in UTC" />}
          />
        </SettingsGroup>
      </SettingsSection>
      <SettingsSection title="Security">
        <SettingsGroup>
          <SettingsRow title="Two-step verification" description="Codes from an authenticator app." control={<Button size="sm">Set up</Button>} />
          <SettingsRow
            title="Single sign-on"
            description="Managed by your organization."
            control={
              <Link href="#" external>
                Open settings
              </Link>
            }
          />
        </SettingsGroup>
      </SettingsSection>
    </div>
  );
}
export function Navigation() {
  return (
    <div className="w-720">
      <SettingsGroup>
        <SettingsRow icon={icons.members} title="Members" description="Who can use this organization." value="5 members" href="#" />
        <SettingsRow icon={icons.providerKey} title="Provider keys" description="Credentials the gateway uses upstream." value="2" href="#" />
        <SettingsRow icon={icons.security} title="Security" description="SSO, two-step verification and key lifetime." value="Off" href="#" />
      </SettingsGroup>
    </div>
  );
}
export function States() {
  return (
    <StateSection title="Groups">
      <div className="flex flex-col gap-24">
        <Preferences />
        <Navigation />
        <Narrow />
        <div className="w-720">
          <SettingsGroup>
            <SettingsRow title="Hover" description="A navigation row under the pointer." value="Value" href="#" data-force="hover" />
            <SettingsRow title="Focus" value="Value" href="#" data-force="focus" />
            <SettingsRow
              title="Organization id"
              description="Used by the Platform API."
              control={<InlineCopyText value="org_01m5btmm86ee096rh834eenxs1" label="Copy organization id" />}
            />
            <SettingsRow
              title="Delete organization"
              description="Projects, keys and logs are deleted after 30 days."
              danger
              control={
                <Button size="sm" variant="danger-secondary">
                  Delete
                </Button>
              }
            />
          </SettingsGroup>
        </div>
      </div>
    </StateSection>
  );
}
export function Narrow() {
  return (
    <div className="w-360">
      <SettingsGroup>
        <SettingsRow
          title="Name"
          description="Shown to the members of your organizations."
          control={
            <div className="flex items-center gap-8">
              <div className="w-200">
                <Input aria-label="Name" size="sm" defaultValue="Ada Okafor" />
              </div>
              <Button size="sm">Save</Button>
            </div>
          }
        />
        <SettingsRow
          title="Account id"
          description="Used by instance administrators and in the audit log."
          control={<InlineCopyText value="usr_01m4sprwt3eksbpepefnnzdg4a" label="Copy account id" />}
        />
        <SettingsRow title="Email" description="owner@example.com" control={<Button size="sm">Change email</Button>} />
        <SettingsRow title="Show times in UTC" description="Request logs and the audit log use UTC." control={<Switch aria-label="Show times in UTC" />} />
      </SettingsGroup>
    </div>
  );
}
