import { useMutation, useQueryClient, type QueryKey, type UseMutationOptions } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";
import { toast } from "~/shell/toast";
import { toastApiError } from "~/api/toast-error";
import { ApiError, isApiError } from "~/api/errors";
import { t } from "~/i18n";
import { requestRecentSignIn } from "./recent-sign-in-store";
import { sessionQuery } from "./session";

export interface ApiMutationOptions<TData, TVars> extends Omit<UseMutationOptions<TData, ApiError, TVars>, "mutationFn" | "onError"> {
  mutationFn: (vars: TVars) => Promise<TData>;
  invalidates?: QueryKey[] | undefined;
  onError?: ((error: ApiError, vars: TVars) => void) | undefined;
}

export function useApiMutation<TData, TVars = void>({ mutationFn, invalidates, onSuccess, onError, ...rest }: ApiMutationOptions<TData, TVars>) {
  const queryClient = useQueryClient();
  const router = useRouter();
  return useMutation<TData, ApiError, TVars>({
    ...rest,
    mutationFn: async (vars) => {
      try {
        return await mutationFn(vars);
      } catch (error) {
        if (isApiError(error) && error.type === "recent_sign_in_required" && (await requestRecentSignIn())) {
          return await mutationFn(vars);
        }
        throw error;
      }
    },
    onSuccess: async (data, vars, ctx, mctx) => {
      for (const queryKey of [...(invalidates ?? []), ["audit-log"]]) void queryClient.invalidateQueries({ queryKey });
      await onSuccess?.(data, vars, ctx, mctx);
    },
    onError: async (error, vars) => {
      if (!isApiError(error)) {
        toast.error(t("auth.unexpectedError", "The change wasn't made: the console hit an unexpected error. Reload the page and try again."));
        return;
      }
      if (error.type === "authentication") {
        await queryClient.invalidateQueries({ queryKey: sessionQuery.queryKey });
        const next = router.state.location.href;
        await router.navigate({ to: "/sign-in", search: { next } });
        return;
      }
      if (error.type === "recent_sign_in_required") return;
      toastApiError(error);
      onError?.(error, vars);
    },
  });
}
