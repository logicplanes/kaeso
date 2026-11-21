export function edgeColor(color: string): string | undefined {
  const soft = /^var\(--k-viz-(\d)-soft\)$/.exec(color);
  return soft ? `var(--k-viz-${soft[1]})` : undefined;
}
