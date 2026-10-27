import { DisplayOptions } from "@logicplanes/kaeso-ui";
import { useLogDisplayOptions } from "~/lib/display-options";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "DisplayOptions",
  description: "View, grouping and ordering, toggles, and the properties to show, behind one circle at the end of a toolbar.",
  group: "Components",
  entry: "display-options",
  import: "DisplayOptions",
  a11y: ["The trigger is named “Display options”; the popover holds labelled controls."],
};
export const examples = [{ id: "basic", title: "Basic", fn: "Basic" }];
export function Basic() {
  return <DisplayOptions {...useLogDisplayOptions()} />;
}
export function States() {
  const options = useLogDisplayOptions();
  return (
    <>
      <StateSection title="Panel">
        <DisplayOptions inline {...options} />
      </StateSection>
      <StateSection title="Toggles only">
        <DisplayOptions inline toggles={options.toggles} />
      </StateSection>
    </>
  );
}
