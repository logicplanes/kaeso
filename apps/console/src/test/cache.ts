import { queryClient } from "~/query";

export const cachedText = (): string =>
  JSON.stringify([
    ...queryClient
      .getMutationCache()
      .getAll()
      .map((m) => m.state),
    ...queryClient
      .getQueryCache()
      .getAll()
      .map((q) => [q.queryKey, q.state.data]),
  ]);
