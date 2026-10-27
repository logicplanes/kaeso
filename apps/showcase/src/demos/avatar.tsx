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
const SURFACES = [
  { label: "Panel", className: "" },
  { label: "Row hover", className: "bg-fill-1 controls-on-fill" },
  { label: "Selected row", className: "bg-fill-2 controls-on-fill" },
  { label: "Card", className: "bg-raised shadow-xs" },
];
export function States() {
  return (
    <>
      <StateSection title="Sizes; a picture, a picture that failed, one-word and Chinese names">
        <div className="flex flex-col gap-16">
          <Sizes />
          <div className="flex items-center gap-12">
            <Avatar name="Ada Okafor" src="/demos/receipt.svg" size={32} />
            <Avatar name="Lin Park" src="/demos/missing-picture.png" size={32} />
            <Avatar name="Northwind" size={32} />
            <Avatar name="李娜" size={32} />
            <Avatar name="Zhang Wei" size={32} />
          </div>
        </div>
      </StateSection>
      <StateSection title="Groups on every surface they sit on">
        <div className="flex flex-col gap-8">
          {SURFACES.map((surface) => (
            <div key={surface.label} className={`flex h-44 w-400 items-center justify-between rounded-item px-12 ${surface.className}`}>
              <span className="text-small text-fg-secondary">{surface.label}</span>
              <AvatarGroup people={members} max={3} size={24} />
            </div>
          ))}
          <div className="flex items-center gap-16">
            {([20, 24, 28, 32, 40] as const).map((s) => (
              <AvatarGroup key={s} people={members} max={2} size={s} />
            ))}
          </div>
        </div>
      </StateSection>
    </>
  );
}
