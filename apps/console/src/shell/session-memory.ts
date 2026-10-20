import { queryClient } from "~/query";

const ROOT = "session-memory";
queryClient.setQueryDefaults([ROOT], { gcTime: Infinity, staleTime: Infinity });

export function recall<T>(name: readonly string[]): T | undefined {
  return queryClient.getQueryData<T>([ROOT, ...name]);
}

export function remember<T>(name: readonly string[], value: T): void {
  queryClient.setQueryData([ROOT, ...name], value);
}
