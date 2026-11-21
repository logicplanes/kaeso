export function edgeColor(color: string): string | undefined {
  const soft = /^var\(--k-viz-(\d)-soft\)$/.exec(color);
  return soft ? `var(--k-viz-${soft[1]})` : undefined;
}

// Inter's tabular figures give a hyphen a figure's width, so a name such as "gpt-5-mini" reads spaced out:
// only labels that are numbers or dates take them.
const NUMERIC = /^[^\p{L}]*\d/u;
export const isNumeric = (label: string) => NUMERIC.test(label);
