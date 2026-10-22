import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { SignInPage } from "~/features/sign-in/sign-in-page";
import { takeFragment } from "~/auth/fragment";
import { internalPath, safeNext } from "~/auth/next";
import { sessionQuery } from "~/auth/session-queries";
import { signInMethodsQuery, type SignInMethods } from "~/auth/sign-in-api";
import { t } from "~/i18n";

const NOTICES: Record<string, () => string> = {
  "password-changed": () => t("auth.notice.passwordChanged", "Password changed; sign in with the new one"),
  "signed-out": () => t("auth.notice.signedOut", "You're signed out"),
};

type SignInSearch = { next?: string; done?: string };

const signInSearch = (search: Record<string, unknown>): SignInSearch => ({
  ...(typeof search.next === "string" && internalPath(search.next) ? { next: search.next } : {}),
  ...(typeof search.done === "string" ? { done: search.done } : {}),
});

export const Route = createFileRoute("/sign-in")({
  validateSearch: signInSearch,
  beforeLoad: async ({ context, search }) => {
    const current = await context.queryClient.fetchQuery({ ...sessionQuery, staleTime: 0, retry: false }).then(
      (who) => who,
      () => null,
    );
    if (!current) return;
    const failed = new URLSearchParams(window.location.hash.slice(1)).get("sso_error");
    const next = safeNext(search.next);
    throw redirect({ href: failed ? `${next}#sso_error=${encodeURIComponent(failed)}` : next, replace: true });
  },
  component: SignIn,
});

function SignIn() {
  const search = Route.useSearch({ select: signInSearch, structuralSharing: true });
  const done = search.done && Object.hasOwn(NOTICES, search.done) ? search.done : undefined;
  const [ssoError] = useState(() => takeFragment("sso_error"));
  const queryClient = useQueryClient();
  const [page, setPage] = useState<SignInMethods | null | undefined>(() => queryClient.getQueryData(signInMethodsQuery.queryKey));
  useEffect(() => {
    let current = true;
    queryClient.fetchQuery(signInMethodsQuery).then(
      (methods) => current && setPage(methods),
      () => current && setPage(null),
    );
    return () => {
      current = false;
    };
  }, [queryClient]);
  return <SignInPage next={search.next} page={page} ssoError={ssoError} notice={done ? NOTICES[done]?.() : undefined} />;
}
