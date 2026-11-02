import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";
import { isApiError } from "~/api/errors";
import { forgetUserData } from "./user-data";

type Router = ReturnType<typeof useRouter>;

export const sessionEnded = (error: unknown): boolean => isApiError(error) && error.type === "authentication";

export async function signInAgain(router: Router): Promise<void> {
  forgetUserData();
  if (router.latestLocation.pathname === "/sign-in") return;
  await router.navigate({ to: "/sign-in", search: { next: router.state.location.href } });
}

export function SignInWhenSessionEnds(): null {
  const queryClient = useQueryClient();
  const router = useRouter();
  useEffect(() => {
    let leaving = false;
    return queryClient.getQueryCache().subscribe((event) => {
      if (leaving || event.type !== "updated" || event.action.type !== "error" || !sessionEnded(event.action.error)) return;
      leaving = true;
      void signInAgain(router).finally(() => (leaving = false));
    });
  }, [queryClient, router]);
  return null;
}
