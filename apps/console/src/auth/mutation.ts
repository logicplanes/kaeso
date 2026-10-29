import { useId } from "react";
import { useMutation, useQueryClient, type InvalidateQueryFilters, type UseMutationOptions } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";
import { toast } from "~/shell/toast";
import { toastApiError } from "~/api/toast-error";
import { ApiError, isApiError } from "~/api/errors";
import { invalidationFilters, type Invalidation } from "~/api/queries/invalidation";
import { t } from "~/i18n";
import { requestRecentSignIn } from "./recent-sign-in-store";
import { forgetUserData } from "~/auth/user-data";
import { sessionQuery } from "./session";

export interface ApiMutationOptions<TData, TVars> extends Omit<UseMutationOptions<TData, ApiError, TVars>, "mutationFn" | "onError"> {
  mutationFn: (vars: TVars) => Promise<TData>;
  invalidates?: Invalidation[] | ((vars: TVars) => Invalidation[]) | undefined;
  onError?: ((error: ApiError, vars: TVars) => void) | undefined;
}

export async function withSignInAgain<T>(run: () => Promise<T>): Promise<T> {
  try {
    return await run();
  } catch (error) {
    if (isApiError(error) && error.type === "recent_sign_in_required" && (await requestRecentSignIn())) return await run();
    if (isApiError(error) && error.type === "other_sign_in_required" && (await requestRecentSignIn("without-sso"))) return await run();
    throw error;
  }
}

// One error toast per change: trying the same change again replaces it, and the change going
// through takes it away, so an error never outlives what it was about.
const errorToastId = (scope: string, vars: unknown) => {
  try {
    return `${scope}:${JSON.stringify(vars) ?? ""}`;
  } catch {
    return scope;
  }
};

export function useApiMutation<TData, TVars = void>({ mutationFn, invalidates, onSuccess, onError, ...rest }: ApiMutationOptions<TData, TVars>) {
  const queryClient = useQueryClient();
  const router = useRouter();
  const scope = useId();
  const filtersFor = (vars: TVars): InvalidateQueryFilters[] =>
    (typeof invalidates === "function" ? invalidates(vars) : (invalidates ?? [])).map(invalidationFilters);
  return useMutation<TData, ApiError, TVars>({
    ...rest,
    mutationFn: (vars) => withSignInAgain(() => mutationFn(vars)),
    onSuccess: async (data, vars, ctx, mctx) => {
      toast.dismiss(errorToastId(scope, vars));
      for (const filters of [...filtersFor(vars), { queryKey: ["audit-log"] }]) void queryClient.invalidateQueries(filters);
      await onSuccess?.(data, vars, ctx, mctx);
    },
    onError: async (error, vars) => {
      const id = errorToastId(scope, vars);
      if (!isApiError(error)) {
        toast.error(t("auth.unexpectedError", "The change wasn't made; reload the page and try again."), { id });
        return;
      }
      if (error.type === "authentication") {
        forgetUserData();
        await queryClient.invalidateQueries({ queryKey: sessionQuery.queryKey });
        const next = router.state.location.href;
        await router.navigate({ to: "/sign-in", search: { next } });
        return;
      }
      if (error.type === "recent_sign_in_required" || error.type === "other_sign_in_required") return;
      if (error.status === 412) {
        for (const filters of filtersFor(vars)) void queryClient.invalidateQueries(filters);
        toastApiError(error, { id, description: t("auth.reloaded", "The latest version is loaded now.") });
        onError?.(error, vars);
        return;
      }
      toastApiError(error, { id });
      onError?.(error, vars);
    },
  });
}
