export function edgeColor(color: string): string | undefined {
  const soft = /^var\(--k-viz-(\d)-soft\)$/.exec(color);
  return soft ? `var(--k-viz-${soft[1]})` : undefined;
}

// Inter's tabular figures give a hyphen a figure's width, so a name such as "gpt-5-mini" reads spaced out.
// Dates, times and numbers keep them, so their figures line up as the pointer moves.
const HYPHENATED_NAME = /\p{L}-|-\p{L}/u;
export const tabularLabel = (label: string) => !HYPHENATED_NAME.test(label);
