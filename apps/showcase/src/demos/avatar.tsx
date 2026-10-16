import { Avatar, AvatarGroup } from "@logicplanes/kaeso-ui";
import { members } from "~/fixtures/kaeso";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Avatar",
  description: "People round with initials or a picture; organizations and projects at radius 8.",
  group: "Components",
  entry: "avatar",
  import: "Avatar, AvatarGroup",
};
export const examples = [
  { id: "sizes", title: "Sizes", fn: "Sizes" },
  { id: "group", title: "Group", fn: "Group" },
];
export function Sizes() {
  return (
    <div className="flex flex-wrap items-end gap-12">
      {([20, 24, 28, 32, 40] as const).map((s) => (
        <Avatar key={s} name="Ada Okafor" size={s} />
      ))}
      {([20, 24, 32, 40] as const).map((s) => (
        <Avatar key={`o${s}`} name="Northwind Labs" kind="org" size={s} />
      ))}
    </div>
  );
}
export function Group() {
  return <AvatarGroup people={members} max={3} size={28} />;
}
export function States() {
  return (
    <StateSection title="Sizes and groups">
      <div className="flex flex-col gap-16">
        <Sizes />
        <Group />
      </div>
    </StateSection>
  );
}
