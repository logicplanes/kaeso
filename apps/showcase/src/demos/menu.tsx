import { Button, Menu, Avatar, IconButton, icons } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Menu",
  description: "A rounded dropdown: 16px radius, 34px rows, icons that wake on highlight.",
  group: "Components",
  entry: "menu",
  import: "Menu",
  keyboard: [
    ["Enter Space", "Opens and activates"],
    ["Up Down", "Moves; wraps at the ends"],
    ["Home End", "First and last item"],
    ["Type", "Jumps to an item"],
    ["Right", "Opens a submenu"],
    ["Escape", "Closes and returns focus"],
  ],
  a11y: [
    "role=menu with menuitem, menuitemcheckbox and menuitemradio.",
    "Groups of more than seven items get a caption or a separator.",
    "Danger items name the action: “Delete key”.",
    "An item's shortcut and hint describe it; its name is the label alone.",
  ],
};
export const examples = [
  { id: "actions", title: "Row actions", fn: "Actions" },
  { id: "account", title: "Account menu", fn: "Account" },
  { id: "choices", title: "Checkbox and radio items", fn: "Choices" },
  { id: "submenu", title: "Submenu", fn: "Submenu" },
];
export function Actions() {
  return (
    <Menu>
      <Menu.Trigger>
        <IconButton icon={icons.more} aria-label="Key actions" variant="secondary" />
      </Menu.Trigger>
      <Menu.Content align="end">
        <Menu.Item icon={icons.edit}>Rename</Menu.Item>
        <Menu.Item icon={icons.copy}>Copy key id</Menu.Item>
        <Menu.Item icon={icons.limit}>Edit limits</Menu.Item>
        <Menu.Separator />
        <Menu.Item icon={icons.hide}>Disable</Menu.Item>
        <Menu.Item variant="danger">Archive key</Menu.Item>
      </Menu.Content>
    </Menu>
  );
}
export function Account() {
  return (
    <Menu>
      <Menu.Trigger>
        <Button variant="ghost" leadingIcon={icons.member}>
          Ada Okafor
        </Button>
      </Menu.Trigger>
      <Menu.Content
        header={<Menu.Header avatar={<Avatar name="Ada Okafor" size={40} decorative />} name="Ada Okafor" email="ada@northwind.example" />}
        footer="Kaeso 0.4.0 · self-hosted"
      >
        <Menu.Separator />
        <Menu.Item icon={icons.settings}>Account settings</Menu.Item>
        <Menu.LinkItem icon={icons.help} href="https://github.com/logicplanes/kaeso" external>
          Documentation
        </Menu.LinkItem>
        <Menu.Separator />
        <Menu.Item icon={icons.signOut}>Sign out</Menu.Item>
      </Menu.Content>
    </Menu>
  );
}
export function Choices() {
  return (
    <Menu>
      <Menu.Trigger>
        <Button trailingIcon={icons.chevronDown}>Columns</Button>
      </Menu.Trigger>
      <Menu.Content>
        <Menu.Group label="Show">
          <Menu.CheckboxItem defaultChecked>Model</Menu.CheckboxItem>
          <Menu.CheckboxItem defaultChecked>Latency</Menu.CheckboxItem>
          <Menu.CheckboxItem>Provider</Menu.CheckboxItem>
        </Menu.Group>
        <Menu.Separator />
        <Menu.Group label="Density">
          <Menu.RadioGroup defaultValue="default">
            <Menu.RadioItem value="compact">Compact</Menu.RadioItem>
            <Menu.RadioItem value="default">Default</Menu.RadioItem>
            <Menu.RadioItem value="comfortable">Comfortable</Menu.RadioItem>
          </Menu.RadioGroup>
        </Menu.Group>
      </Menu.Content>
    </Menu>
  );
}
export function Submenu() {
  return (
    <Menu>
      <Menu.Trigger>
        <Button>Move to project</Button>
      </Menu.Trigger>
      <Menu.Content>
        <Menu.Item icon={icons.project}>Checkout assistant</Menu.Item>
        <Menu.Sub>
          <Menu.SubTrigger icon={icons.organization}>Other organizations</Menu.SubTrigger>
          <Menu.SubContent>
            <Menu.Item>Contoso</Menu.Item>
            <Menu.Item>Fabrikam</Menu.Item>
          </Menu.SubContent>
        </Menu.Sub>
      </Menu.Content>
    </Menu>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Open menu (items forced: highlighted, pressed, danger, disabled)">
        <div className="h-560">
          <Menu open>
            <Menu.Trigger>
              <Button data-force="open">Key actions</Button>
            </Menu.Trigger>
            <Menu.Content>
              <Menu.Group label="Key">
                <Menu.Item icon={icons.edit} shortcut={["e"]}>
                  Rename
                </Menu.Item>
                <Menu.Item icon={icons.copy} data-force="highlighted">
                  Copy key id
                </Menu.Item>
                <Menu.Item icon={icons.limit} data-force="pressed">
                  Edit limits
                </Menu.Item>
                <Menu.Item icon={icons.limit} description="Rates, budgets and model access.">
                  Edit access
                </Menu.Item>
                <Menu.Item icon={icons.refresh} disabled description="Rotation comes in a later release.">
                  Rotate
                </Menu.Item>
              </Menu.Group>
              <Menu.Separator />
              <Menu.Item variant="danger">Archive key</Menu.Item>
              <Menu.Item icon={icons.delete} variant="danger" data-force="highlighted">
                Delete key
              </Menu.Item>
              <Menu.Item icon={icons.delete} variant="danger" data-force="pressed">
                Delete every key in this project
              </Menu.Item>
            </Menu.Content>
          </Menu>
        </div>
      </StateSection>
      <StateSection title="Checkbox and radio items, an open submenu">
        <div className="h-320">
          <Menu open>
            <Menu.Trigger>
              <Button data-force="open" trailingIcon={icons.chevronDown}>
                Columns
              </Button>
            </Menu.Trigger>
            <Menu.Content>
              <Menu.Group label="Show">
                <Menu.CheckboxItem defaultChecked>Model</Menu.CheckboxItem>
                <Menu.CheckboxItem>Provider</Menu.CheckboxItem>
              </Menu.Group>
              <Menu.Separator />
              <Menu.Group label="Density">
                <Menu.RadioGroup defaultValue="default">
                  <Menu.RadioItem value="compact">Compact</Menu.RadioItem>
                  <Menu.RadioItem value="default">Default</Menu.RadioItem>
                </Menu.RadioGroup>
              </Menu.Group>
            </Menu.Content>
          </Menu>
        </div>
        <div className="h-200">
          <Menu open>
            <Menu.Trigger>
              <Button data-force="open">Move to project</Button>
            </Menu.Trigger>
            <Menu.Content>
              <Menu.Item icon={icons.project}>Checkout assistant</Menu.Item>
              <Menu.Sub open>
                <Menu.SubTrigger icon={icons.organization}>Other organizations</Menu.SubTrigger>
                <Menu.SubContent>
                  <Menu.Item>Contoso</Menu.Item>
                  <Menu.Item>Fabrikam</Menu.Item>
                </Menu.SubContent>
              </Menu.Sub>
            </Menu.Content>
          </Menu>
        </div>
      </StateSection>
      <StateSection title="Header, footer, an external link and a long label">
        <div className="h-480">
          <Menu open>
            <Menu.Trigger>
              <Button data-force="open" variant="ghost" leadingIcon={icons.member}>
                Ada Okafor
              </Button>
            </Menu.Trigger>
            <Menu.Content
              header={
                <Menu.Header avatar={<Avatar name="Ada Okafor" size={40} decorative />} name="Ada Okafor" email="ada.okafor@northwind-laboratories.example" />
              }
              footer="Kaeso · self-hosted"
            >
              <Menu.Item icon={icons.project}>Checkout assistant for the European storefront and its partners</Menu.Item>
              <Menu.Item icon={icons.settings}>Account settings</Menu.Item>
              <Menu.LinkItem icon={icons.help} href="https://github.com/logicplanes/kaeso" external>
                Documentation
              </Menu.LinkItem>
              <Menu.Separator />
              <Menu.Item icon={icons.signOut}>Sign out</Menu.Item>
            </Menu.Content>
          </Menu>
        </div>
      </StateSection>
    </>
  );
}
