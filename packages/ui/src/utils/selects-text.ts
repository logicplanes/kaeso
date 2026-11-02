export function selectsText(row: HTMLElement): boolean {
  const selection = typeof window === "undefined" ? null : window.getSelection();
  return !!selection && !selection.isCollapsed && !!selection.anchorNode && row.contains(selection.anchorNode);
}
