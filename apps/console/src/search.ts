export type SearchPatch<S> = { [K in keyof S]?: S[K] | undefined };

export function patchSearch<S extends object>(search: S, patch: SearchPatch<S>, drop: readonly (keyof S)[] = []): S {
  const merged: SearchPatch<S> = { ...search, ...patch };
  const kept = Object.entries(merged).filter(([key, value]) => !drop.includes(key as keyof S) && value !== undefined && value !== "");
  return Object.fromEntries(kept) as S;
}
