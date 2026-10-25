import type * as React from "react";
import { Grid, Stack, Inline } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Grid",
  description: "Grid, Stack and Inline: layout with gaps from the spacing scale only.",
  group: "Components",
  entry: "layout",
  import: "Grid, Stack, Inline",
  variants: "LAYOUT",
};
const Box = ({ children }: { children: React.ReactNode }) => (
  <div className="flex h-40 items-center justify-center rounded-item bg-fill-1 text-small text-fg-secondary">{children}</div>
);
export const examples = [
  { id: "grid", title: "Grid", fn: "GridDemo" },
  { id: "stack", title: "Stack and Inline", fn: "StackDemo" },
];
export function GridDemo() {
  return (
    <Grid columns={3} gap={12} className="w-full">
      {[1, 2, 3, 4, 5, 6].map((i) => (
        <Box key={i}>{i}</Box>
      ))}
    </Grid>
  );
}
export function StackDemo() {
  return (
    <Stack gap={12} className="w-320">
      <Box>Stack</Box>
      <Inline gap={8}>
        <Box>Inline</Box>
        <Box>items</Box>
      </Inline>
    </Stack>
  );
}
export function States() {
  return (
    <StateSection title="Columns">
      <div className="flex w-640 flex-col gap-24">
        {([2, 3, 4] as const).map((c) => (
          <Grid key={c} columns={c} gap={12}>
            {Array.from({ length: c }, (_, i) => (
              <Box key={i}>{i + 1}</Box>
            ))}
          </Grid>
        ))}
      </div>
    </StateSection>
  );
}
