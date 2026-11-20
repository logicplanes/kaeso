// A control marked with this property keeps its place at a settings row's end on a phone. It's a property the build keeps:
// `displayName` exists in development only.
export const COMPACT_ROW_CONTROL = "kaesoCompactRowControl";

export function isCompactRowControl(type: unknown): boolean {
  return (typeof type === "object" || typeof type === "function") && type !== null && (type as Record<string, unknown>)[COMPACT_ROW_CONTROL] === true;
}
