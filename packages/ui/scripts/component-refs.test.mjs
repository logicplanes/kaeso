import { describe, expect, it } from "vitest";
import { componentsWithoutRef } from "./component-refs.mjs";

const names = (source) => componentsWithoutRef("example.tsx", source).map((c) => c.name);

describe("componentsWithoutRef", () => {
  it("names exported function components that render an element", () => {
    expect(
      componentsWithoutRef(
        "example.tsx",
        `export function Swatch({ color }: { color: string }) {
  return <span style={{ background: color }} />;
}
export const Live = ({ children }: { children: ReactNode }) => <div aria-live="polite">{children}</div>;`,
      ),
    ).toEqual([
      { name: "Swatch", line: 1 },
      { name: "Live", line: 4 },
    ]);
  });

  it("accepts forwardRef components, with or without type arguments", () => {
    expect(
      names(`export const Swatch = forwardRef<HTMLSpanElement, SwatchProps>(function Swatch(props, ref) {
  return <span ref={ref} />;
});
export const Spacer = forwardRef(function Spacer(_props, ref) {
  return <span ref={ref} />;
});`),
    ).toEqual([]);
  });

  it("ignores functions that aren't exported, aren't components or render no JSX", () => {
    expect(
      names(`function Inner() {
  return <span />;
}
export function useThing() {
  return <span />;
}
export function Format(value: number) {
  return String(value);
}`),
    ).toEqual([]);
  });

  it("names plain members of an exported compound component", () => {
    expect(
      names(`const Root = forwardRef<HTMLDivElement, Props>(function Toolbar(props, ref) {
  return <div ref={ref} />;
});
function Spacer() {
  return <span />;
}
export const Toolbar = Object.assign(Root, { Spacer, Inline: function Inline() { return <span />; } });`),
    ).toEqual(["Spacer", "Inline"]);
  });

  it("names a plain base of a compound component and components exported by name or alias", () => {
    expect(
      names(`function Base() {
  return <div />;
}
export const Toast = Object.assign(Base, { add });
function Card() {
  return <div />;
}
export { Card as Tile };
function Page() {
  return <main />;
}
export const ListPage = Page;`),
    ).toEqual(["Base", "Card", "Page"]);
  });
});
