// The key lists' sort, kept apart from their search so a route reads it without pulling the list
// code into the entry chunk.

/** How a list of keys is ordered besides by name, its default. */
export type KeySort = "used" | "expiry";

export const isKeySort = (v: unknown): v is KeySort => v === "used" || v === "expiry";
