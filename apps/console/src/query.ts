import { QueryCache, QueryClient } from "@tanstack/react-query";
import { isApiError } from "./api/errors";
import { toastApiError } from "./api/toast-error";
import { connectionWatched } from "./shell/connection-watch";
import { t } from "./i18n";

export const queryClient = new QueryClient({
  queryCache: new QueryCache({
    onError: (error, query) => {
      if (!isApiError(error) || error.type === "authentication" || error.type === "not_found") return;
      if (query.meta?.["quiet"] || query.getObserversCount() === 0) return;
      if (error.unreachable && connectionWatched()) return;
      toastApiError(error, {
        id: `load:${error.type}:${error.message}`,
        action: { label: t("common.tryAgain", "Try again"), onClick: () => void query.fetch() },
      });
    },
  }),
  defaultOptions: {
    queries: {
      staleTime: 10_000,
      gcTime: 10 * 60_000,
      refetchOnWindowFocus: true,
      retry: (count, error) => (isApiError(error) && error.status >= 400 && error.status < 500 ? false : count < 2),
    },
    mutations: { retry: false },
  },
});
